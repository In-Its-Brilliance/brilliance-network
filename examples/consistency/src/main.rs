use clap::Parser;
use log::LevelFilter;
use network::{
    client::IClientNetwork,
    messages::{ClientMessages, NetworkMessageType, ServerMessages},
    server::{ConnectionMessages, IServerConnection, IServerNetwork},
    NetworkClient, NetworkServer, NetworkServerConnection,
};
use std::time::{Duration, Instant};

/// Тест консистентности доставки unreliable сообщений через сетевой слой.
///
/// Клиент шлёт PlayerMove каждые ~16мс (64 TPS).
/// Сервер принимает и логирует: сколько сообщений за тик, интервалы, батчинг.
/// Сервер также отправляет EntityMove обратно клиенту для проверки обратного пути.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    #[arg(short, long, default_value_t = String::from("127.0.0.1:25570"))]
    ip: String,

    #[arg(short = 't', long, default_value_t = String::from("server"))]
    run_type: String,

    /// Длительность теста в секундах
    #[arg(short, long, default_value_t = 10)]
    duration: u64,
}

struct SimpleLogger;
impl log::Log for SimpleLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, record: &log::Record) {
        eprintln!("{} {}", record.level(), record.args());
    }
    fn flush(&self) {}
}
static LOGGER: SimpleLogger = SimpleLogger;

#[tokio::main]
async fn main() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(LevelFilter::Info);

    let args = Args::parse();

    if args.run_type == "server" {
        run_server(args).await;
    } else {
        run_client(args).await;
    }
}

// ============================================================
// SERVER
// ============================================================

async fn run_server(args: Args) {
    log::info!("Server starting on {}", args.ip);
    let server = NetworkServer::new(args.ip.clone()).await;

    let tick_duration = Duration::from_secs_f64(1.0 / 64.0);
    let test_end = Instant::now() + Duration::from_secs(args.duration);

    let mut connection: Option<NetworkServerConnection> = None;

    // Статистика
    let mut recv_counts: Vec<usize> = Vec::new(); // сколько сообщений за каждый тик
    let mut recv_timestamps: Vec<Instant> = Vec::new(); // время получения каждого сообщения
    let mut recv_sequences: Vec<u64> = Vec::new(); // порядковые номера
    let mut total_ticks: u64 = 0;
    let mut entity_move_sent: u64 = 0;

    log::info!("Waiting for client connection...");

    loop {
        let tick_start = Instant::now();

        server.step(tick_duration).await;

        for error in server.drain_errors() {
            log::error!("Server error: {}", error);
        }

        // Обработка подключений
        for msg in server.drain_connections() {
            match msg {
                ConnectionMessages::Connect { connection: conn } => {
                    log::info!("Client connected: id={}", conn.get_client_id());
                    conn.send_message(
                        NetworkMessageType::ReliableOrdered,
                        &ServerMessages::AllowConnection,
                    );
                    connection = Some(conn);
                }
                ConnectionMessages::Disconnect { client_id, reason } => {
                    log::info!("Client disconnected: id={} reason={}", client_id, reason);
                    connection = None;
                }
            }
        }

        // Получение сообщений
        if let Some(conn) = connection.as_ref() {
            let mut count_this_tick = 0u64;
            let now = Instant::now();

            for msg in conn.drain_client_messages() {
                match msg {
                    ClientMessages::PlayerMove { position, rotation } => {
                        count_this_tick += 1;
                        let seq = position.x as u64; // sequence закодирован в position.x
                        recv_timestamps.push(now);
                        recv_sequences.push(seq);

                        // Отправляем EntityMove обратно
                        conn.send_message(
                            NetworkMessageType::Unreliable,
                            &ServerMessages::EntityMove {
                                world_slug: String::new(),
                                id: 1,
                                position,
                                rotation,
                                timestamp: tick_start.elapsed().as_secs_f32(),
                            },
                        );
                        entity_move_sent += 1;
                    }
                    ClientMessages::ConnectionInfo { .. } => {
                        log::info!("Client sent ConnectionInfo");
                    }
                    _ => {}
                }
            }

            if connection.is_some() {
                recv_counts.push(count_this_tick as usize);
                total_ticks += 1;
            }
        }

        if Instant::now() >= test_end && connection.is_some() {
            break;
        }

        let elapsed = tick_start.elapsed();
        if elapsed < tick_duration {
            tokio::time::sleep(tick_duration - elapsed).await;
        }
    }

    // ============ СТАТИСТИКА ============
    println!("\n========== SERVER RECEIVE STATS ==========");
    println!("Total ticks: {}", total_ticks);
    println!(
        "Total messages received: {}",
        recv_sequences.len()
    );
    println!("EntityMove sent back: {}", entity_move_sent);

    if recv_counts.is_empty() {
        println!("No data received.");
        return;
    }

    // Распределение сообщений по тикам
    let mut distribution: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    for &count in &recv_counts {
        *distribution.entry(count).or_insert(0) += 1;
    }
    println!("\nMessages per tick distribution:");
    for (msgs, ticks) in &distribution {
        let pct = (*ticks as f64 / total_ticks as f64) * 100.0;
        println!(
            "  {} msg/tick: {} ticks ({:.1}%)",
            msgs, ticks, pct
        );
    }

    // Тики с батчингом (>1 сообщение)
    let batched_ticks: usize = recv_counts.iter().filter(|&&c| c > 1).count();
    let empty_ticks: usize = recv_counts.iter().filter(|&&c| c == 0).count();
    println!(
        "\nBatched ticks (>1 msg): {} ({:.1}%)",
        batched_ticks,
        (batched_ticks as f64 / total_ticks as f64) * 100.0
    );
    println!(
        "Empty ticks (0 msg): {} ({:.1}%)",
        empty_ticks,
        (empty_ticks as f64 / total_ticks as f64) * 100.0
    );

    // Проверка порядка
    let mut out_of_order = 0;
    for i in 1..recv_sequences.len() {
        if recv_sequences[i] <= recv_sequences[i - 1] {
            out_of_order += 1;
        }
    }
    println!("Out of order messages: {}", out_of_order);

    // Пропущенные сообщения
    if let (Some(&first), Some(&last)) = (recv_sequences.first(), recv_sequences.last()) {
        let expected = last - first + 1;
        let lost = expected as i64 - recv_sequences.len() as i64;
        println!(
            "Sequence range: {} .. {} (expected {}, got {}, lost {})",
            first,
            last,
            expected,
            recv_sequences.len(),
            lost.max(0)
        );
    }

    // Интервалы между тиками с сообщениями
    let active_tick_times: Vec<Instant> = recv_counts
        .iter()
        .zip(std::iter::successors(Some(Instant::now()), |_| None))
        .collect::<Vec<_>>()
        .into_iter()
        .map(|_| Instant::now())
        .collect();
    let _ = active_tick_times; // timing info already in recv_timestamps

    // Максимальный батч
    if let Some(&max_batch) = recv_counts.iter().max() {
        println!("Max messages in single tick: {}", max_batch);
    }

    println!("==========================================\n");
}

// ============================================================
// CLIENT
// ============================================================

async fn run_client(args: Args) {
    log::info!("Client connecting to {}", args.ip);
    let client = NetworkClient::new(args.ip.clone()).await.unwrap();

    let send_interval = Duration::from_secs_f64(1.0 / 64.0);
    let step_interval = Duration::from_secs_f64(1.0 / 64.0);
    let test_end = Instant::now() + Duration::from_secs(args.duration);

    let mut connected = false;
    let mut sequence: u64 = 0;
    let mut total_sent: u64 = 0;

    // Статистика приёма EntityMove
    let mut recv_counts: Vec<usize> = Vec::new();
    let mut recv_total: u64 = 0;
    let mut total_client_ticks: u64 = 0;

    let mut last_send = Instant::now();

    loop {
        let tick_start = Instant::now();

        client.step(step_interval).await;

        for error in client.iter_errors() {
            log::error!("Client error: {}", error);
            return;
        }

        let mut recv_this_tick = 0usize;
        for msg in client.iter_server_messages() {
            match msg {
                ServerMessages::AllowConnection => {
                    log::info!("Connection allowed, starting test...");
                    client.send_message(
                        NetworkMessageType::ReliableOrdered,
                        &ClientMessages::ConnectionInfo {
                            login: "consistency-test".to_string(),
                            version: "test".to_string(),
                            architecture: "test".to_string(),
                            rendering_device: "test".to_string(),
                        },
                    );
                    connected = true;
                    last_send = Instant::now();
                }
                ServerMessages::EntityMove { .. } => {
                    recv_this_tick += 1;
                    recv_total += 1;
                }
                _ => {}
            }
        }

        if connected {
            recv_counts.push(recv_this_tick);
            total_client_ticks += 1;

            // Отправка PlayerMove с фиксированным интервалом
            if tick_start.duration_since(last_send) >= send_interval {
                sequence += 1;
                let msg = ClientMessages::PlayerMove {
                    position: common::chunks::position::Vector3 {
                        x: sequence as f32,
                        y: 0.0,
                        z: 0.0,
                    },
                    rotation: common::chunks::rotation::Rotation::new(0.0, 0.0),
                };
                client.send_message(NetworkMessageType::Unreliable, &msg);
                total_sent += 1;
                last_send = tick_start;
            }
        }

        if Instant::now() >= test_end && connected {
            break;
        }

        let elapsed = tick_start.elapsed();
        if elapsed < step_interval {
            tokio::time::sleep(step_interval - elapsed).await;
        }
    }

    // ============ СТАТИСТИКА ============
    println!("\n========== CLIENT STATS ==========");
    println!("Total PlayerMove sent: {}", total_sent);
    println!("Total EntityMove received: {}", recv_total);
    println!("Client ticks: {}", total_client_ticks);

    if !recv_counts.is_empty() {
        let mut distribution: std::collections::BTreeMap<usize, usize> =
            std::collections::BTreeMap::new();
        for &count in &recv_counts {
            *distribution.entry(count).or_insert(0) += 1;
        }
        println!("\nEntityMove per tick distribution:");
        for (msgs, ticks) in &distribution {
            let pct = (*ticks as f64 / total_client_ticks as f64) * 100.0;
            println!("  {} msg/tick: {} ticks ({:.1}%)", msgs, ticks, pct);
        }

        let batched = recv_counts.iter().filter(|&&c| c > 1).count();
        println!(
            "\nBatched ticks (>1 msg): {} ({:.1}%)",
            batched,
            (batched as f64 / total_client_ticks as f64) * 100.0
        );

        if let Some(&max_batch) = recv_counts.iter().max() {
            println!("Max messages in single tick: {}", max_batch);
        }
    }

    let lost = total_sent as i64 - recv_total as i64;
    println!(
        "\nMessage loss: {} ({:.1}%)",
        lost.max(0),
        if total_sent > 0 {
            (lost.max(0) as f64 / total_sent as f64) * 100.0
        } else {
            0.0
        }
    );
    println!("==================================\n");
}
