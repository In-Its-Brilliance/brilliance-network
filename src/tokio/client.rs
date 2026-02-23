use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::utils::debug::info::{DebugInfo, DebugValue};
use flume::Drain;
use parking_lot::{Mutex, RwLock, RwLockReadGuard};
use tokio::io::{AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

use crate::client::{resolve_connect_domain, IClientNetwork};
use crate::messages::{ClientMessages, NetworkMessageType, ServerMessages};

use super::{read_frame, write_frame, FRAME_MESSAGE, FRAME_PING, FRAME_PONG};

pub struct TokioClient {
    connected: Arc<AtomicBool>,
    debug_info: Arc<RwLock<DebugInfo>>,
    rtt_nanos: Arc<AtomicU64>,

    incoming_messages: (flume::Sender<ServerMessages>, flume::Receiver<ServerMessages>),
    incoming_errors: (flume::Sender<String>, flume::Receiver<String>),
    outgoing_messages: (flume::Sender<Vec<u8>>, flume::Receiver<Vec<u8>>),
}

/// Background task: reads length-prefixed frames from the socket,
/// dispatches messages to the incoming channel, handles pong for RTT.
async fn client_reader_task(
    reader: OwnedReadHalf,
    tx: flume::Sender<ServerMessages>,
    error_tx: flume::Sender<String>,
    connected: Arc<AtomicBool>,
    last_ping_sent: Arc<Mutex<Option<Instant>>>,
    rtt_nanos: Arc<AtomicU64>,
) {
    let mut buf_reader = BufReader::new(reader);
    loop {
        match read_frame(&mut buf_reader).await {
            Ok(data) if data.is_empty() => continue,
            Ok(data) => match data[0] {
                FRAME_MESSAGE => match bincode::deserialize::<ServerMessages>(&data[1..]) {
                    Ok(msg) => {
                        if tx.send(msg).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        error_tx
                            .send(format!("Message decode error: {}", e))
                            .ok();
                    }
                },
                FRAME_PONG => {
                    if let Some(sent_at) = last_ping_sent.lock().take() {
                        rtt_nanos.store(sent_at.elapsed().as_nanos() as u64, Ordering::Relaxed);
                    }
                }
                _ => {}
            },
            Err(_) => {
                connected.store(false, Ordering::SeqCst);
                break;
            }
        }
    }
}

/// Background task: drains outgoing channel, writes length-prefixed frames
/// to the socket with batch-flushing. Sends periodic ping frames.
async fn client_writer_task(
    writer: OwnedWriteHalf,
    rx: flume::Receiver<Vec<u8>>,
    connected: Arc<AtomicBool>,
    last_ping_sent: Arc<Mutex<Option<Instant>>>,
) {
    let mut buf_writer = BufWriter::new(writer);
    let ping_start = tokio::time::Instant::now() + Duration::from_secs(1);
    let mut ping_interval = tokio::time::interval_at(ping_start, Duration::from_secs(1));

    loop {
        if !connected.load(Ordering::SeqCst) {
            break;
        }
        tokio::select! {
            result = rx.recv_async() => {
                match result {
                    Ok(data) => {
                        if write_frame(&mut buf_writer, &data).await.is_err() {
                            connected.store(false, Ordering::SeqCst);
                            break;
                        }
                        // Batch any additional queued messages before flushing
                        while let Ok(data) = rx.try_recv() {
                            if write_frame(&mut buf_writer, &data).await.is_err() {
                                connected.store(false, Ordering::SeqCst);
                                return;
                            }
                        }
                        if buf_writer.flush().await.is_err() {
                            connected.store(false, Ordering::SeqCst);
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            _ = ping_interval.tick() => {
                *last_ping_sent.lock() = Some(Instant::now());
                if write_frame(&mut buf_writer, &[FRAME_PING]).await.is_err() {
                    connected.store(false, Ordering::SeqCst);
                    break;
                }
                if buf_writer.flush().await.is_err() {
                    connected.store(false, Ordering::SeqCst);
                    break;
                }
            }
        }
    }
}

impl IClientNetwork for TokioClient {
    async fn new(ip_port: String) -> Result<Self, String> {
        let addr = resolve_connect_domain(&ip_port, 25565).await?;

        let stream = TcpStream::connect(addr)
            .await
            .map_err(|e| format!("Connection to {} failed: {}", addr, e))?;

        stream
            .set_nodelay(true)
            .map_err(|e| format!("Failed to set TCP_NODELAY: {}", e))?;

        let (reader, writer) = stream.into_split();

        let connected = Arc::new(AtomicBool::new(true));
        let rtt_nanos = Arc::new(AtomicU64::new(0));
        let last_ping_sent = Arc::new(Mutex::new(None));
        let incoming_messages = flume::unbounded();
        let incoming_errors = flume::unbounded();
        let outgoing_messages = flume::unbounded();

        // Spawn background reader task
        {
            let tx = incoming_messages.0.clone();
            let error_tx = incoming_errors.0.clone();
            let connected = connected.clone();
            let last_ping_sent = last_ping_sent.clone();
            let rtt_nanos = rtt_nanos.clone();
            tokio::spawn(async move {
                client_reader_task(reader, tx, error_tx, connected, last_ping_sent, rtt_nanos)
                    .await;
            });
        }

        // Spawn background writer task
        {
            let rx = outgoing_messages.1.clone();
            let connected = connected.clone();
            let last_ping_sent = last_ping_sent.clone();
            tokio::spawn(async move {
                client_writer_task(writer, rx, connected, last_ping_sent).await;
            });
        }

        log::info!(target: "network", "Connected to {}", addr);

        Ok(Self {
            connected,
            debug_info: Arc::new(RwLock::new(Default::default())),
            rtt_nanos,
            incoming_messages,
            incoming_errors,
            outgoing_messages,
        })
    }

    async fn step(&self, _delta: Duration) -> bool {
        if !self.connected.load(Ordering::SeqCst) {
            return false;
        }

        let rtt_ns = self.rtt_nanos.load(Ordering::Relaxed);
        let rtt = Duration::from_nanos(rtt_ns);
        let rtt_ms = rtt.as_secs_f64() * 1000.0;
        let ping_color = match rtt_ms {
            ms if ms < 20.0 => "&a",
            ms if ms < 50.0 => "&e",
            ms if ms < 80.0 => "&o",
            ms if ms < 120.0 => "&s",
            ms if ms < 200.0 => "&c",
            _ => "&4",
        };

        let mut debug = self.debug_info.write();
        *debug = DebugInfo::new()
            .insert("is_connected", true)
            .insert("ping", DebugValue::from(rtt).with_color(ping_color));

        true
    }

    fn iter_server_messages(&self) -> Drain<'_, ServerMessages> {
        self.incoming_messages.1.drain()
    }

    fn iter_errors(&self) -> Drain<'_, String> {
        self.incoming_errors.1.drain()
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    fn disconnect(&self) {
        self.connected.swap(false, Ordering::SeqCst);
    }

    fn send_message(&self, _message_type: NetworkMessageType, message: &ClientMessages) {
        if !self.connected.load(Ordering::SeqCst) {
            return;
        }
        let mut frame = vec![FRAME_MESSAGE];
        frame.extend(bincode::serialize(message).unwrap());
        self.outgoing_messages.0.send(frame).ok();
    }

    fn get_debug_info(&self) -> RwLockReadGuard<'_, DebugInfo> {
        self.debug_info.read()
    }
}
