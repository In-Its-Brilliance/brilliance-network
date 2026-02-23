use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tokio::io::{AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpListener;

use crate::messages::{ClientMessages, NetworkMessageType, ServerMessages};
use crate::server::{ConnectionMessages, IServerConnection, IServerNetwork};

use super::{read_frame, write_frame, FRAME_MESSAGE, FRAME_PING, FRAME_PONG};

pub struct TokioServer {
    new_connections_rx: flume::Receiver<(tokio::net::TcpStream, std::net::SocketAddr)>,
    connections: Arc<RwLock<HashMap<u64, TokioServerConnection>>>,

    channel_connections: (
        flume::Sender<ConnectionMessages<TokioServerConnection>>,
        flume::Receiver<ConnectionMessages<TokioServerConnection>>,
    ),
    channel_errors: (flume::Sender<String>, flume::Receiver<String>),
    next_client_id: AtomicU64,
}

/// Background task: reads length-prefixed frames from a client socket,
/// dispatches messages to the connection's channel, responds to ping with pong.
async fn connection_reader_task(
    reader: OwnedReadHalf,
    tx: flume::Sender<ClientMessages>,
    error_tx: flume::Sender<String>,
    connected: Arc<AtomicBool>,
    outgoing_tx: flume::Sender<Vec<u8>>,
) {
    let mut buf_reader = BufReader::new(reader);
    loop {
        match read_frame(&mut buf_reader).await {
            Ok(data) if data.is_empty() => continue,
            Ok(data) => match data[0] {
                FRAME_MESSAGE => match bincode::deserialize::<ClientMessages>(&data[1..]) {
                    Ok(msg) => {
                        if tx.send(msg).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        error_tx
                            .send(format!("Client message decode error: {}", e))
                            .ok();
                    }
                },
                FRAME_PING => {
                    outgoing_tx.send(vec![FRAME_PONG]).ok();
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

/// Background task: drains outgoing channel and writes length-prefixed frames
/// to the client socket with batch-flushing.
async fn connection_writer_task(
    writer: OwnedWriteHalf,
    rx: flume::Receiver<Vec<u8>>,
    connected: Arc<AtomicBool>,
) {
    let mut buf_writer = BufWriter::new(writer);
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
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                // Periodic check for connected flag
                continue;
            }
        }
    }
}

impl IServerNetwork<TokioServerConnection> for TokioServer {
    async fn new(ip_port: String) -> Self {
        let listener = TcpListener::bind(&ip_port).await.unwrap();
        log::info!(target: "network", "TCP server listening on {}", ip_port);

        let (new_conn_tx, new_conn_rx) = flume::unbounded();

        // Spawn background accept loop
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        if new_conn_tx.send((stream, addr)).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        log::error!(target: "network", "Accept error: {}", e);
                    }
                }
            }
        });

        Self {
            new_connections_rx: new_conn_rx,
            connections: Arc::new(RwLock::new(HashMap::new())),
            channel_connections: flume::unbounded(),
            channel_errors: flume::unbounded(),
            next_client_id: AtomicU64::new(1),
        }
    }

    async fn step(&self, _delta: Duration) {
        // Process new connections from the accept loop
        for (stream, addr) in self.new_connections_rx.drain() {
            stream.set_nodelay(true).ok();
            let (reader, writer) = stream.into_split();

            let client_id = self.next_client_id.fetch_add(1, Ordering::SeqCst);
            let connected = Arc::new(AtomicBool::new(true));
            let (msg_tx, msg_rx) = flume::unbounded();
            let (out_tx, out_rx) = flume::unbounded();

            // Spawn per-connection reader task
            {
                let error_tx = self.channel_errors.0.clone();
                let connected = connected.clone();
                let outgoing_tx = out_tx.clone();
                tokio::spawn(async move {
                    connection_reader_task(reader, msg_tx, error_tx, connected, outgoing_tx).await;
                });
            }

            // Spawn per-connection writer task
            {
                let connected = connected.clone();
                tokio::spawn(async move {
                    connection_writer_task(writer, out_rx, connected).await;
                });
            }

            let connection = TokioServerConnection {
                client_id,
                ip: addr.to_string(),
                connected,
                disconnect_at: Arc::new(RwLock::new(None)),
                channel_client_messages: msg_rx,
                channel_outgoing: out_tx,
            };

            self.connections
                .write()
                .insert(client_id, connection.clone());
            self.channel_connections
                .0
                .send(ConnectionMessages::Connect { connection })
                .ok();
        }

        // Handle disconnections (remote close or graceful disconnect delay)
        let mut to_remove = Vec::new();
        {
            let connections = self.connections.read();
            for (&id, conn) in connections.iter() {
                let should_remove = if let Some(at) = *conn.disconnect_at.read() {
                    Instant::now() >= at
                } else {
                    !conn.connected.load(Ordering::SeqCst)
                };

                if should_remove {
                    to_remove.push(id);
                }
            }
        }

        if !to_remove.is_empty() {
            let mut connections = self.connections.write();
            for id in to_remove {
                if let Some(conn) = connections.remove(&id) {
                    conn.connected.store(false, Ordering::SeqCst);
                    self.channel_connections
                        .0
                        .send(ConnectionMessages::Disconnect {
                            client_id: id,
                            reason: "Disconnected".to_string(),
                        })
                        .ok();
                }
            }
        }

        log::trace!(target: "network", "network step");
    }

    fn drain_connections(&self) -> impl Iterator<Item = ConnectionMessages<TokioServerConnection>> {
        self.channel_connections.1.drain()
    }

    fn drain_errors(&self) -> impl Iterator<Item = String> {
        self.channel_errors.1.drain()
    }

    fn is_connected(&self, connection: &TokioServerConnection) -> bool {
        if connection.is_to_disconnect() {
            return false;
        }
        connection.connected.load(Ordering::SeqCst)
    }

    fn connections_count(&self) -> usize {
        self.connections.read().len()
    }
}

#[derive(Clone)]
pub struct TokioServerConnection {
    client_id: u64,
    ip: String,
    connected: Arc<AtomicBool>,
    disconnect_at: Arc<RwLock<Option<Instant>>>,

    channel_client_messages: flume::Receiver<ClientMessages>,
    channel_outgoing: flume::Sender<Vec<u8>>,
}

impl TokioServerConnection {
    fn is_to_disconnect(&self) -> bool {
        if let Some(time) = *self.disconnect_at.read() {
            Instant::now() >= time
        } else {
            false
        }
    }
}

impl IServerConnection for TokioServerConnection {
    fn get_ip(&self) -> &String {
        &self.ip
    }

    fn get_client_id(&self) -> u64 {
        self.client_id
    }

    fn drain_client_messages(&self) -> impl Iterator<Item = ClientMessages> {
        self.channel_client_messages.drain()
    }

    fn send_message(&self, _message_type: NetworkMessageType, message: &ServerMessages) {
        if !self.connected.load(Ordering::SeqCst) {
            return;
        }
        let mut frame = vec![FRAME_MESSAGE];
        frame.extend(bincode::serialize(message).unwrap());
        self.channel_outgoing.send(frame).ok();
    }

    fn disconnect(&self) {
        // Disconnect after 200ms delay to allow pending messages to flush
        let mut disconnect_at = self.disconnect_at.write();
        if disconnect_at.is_none() {
            *disconnect_at = Some(Instant::now() + Duration::from_millis(200));
        }
    }
}
