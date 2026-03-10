use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LogEvent {
    Info { message: String, timestamp: u64 },
    Progress { message: String, current: usize, total: usize, timestamp: u64 },
    Warning { message: String, timestamp: u64 },
    Error { message: String, timestamp: u64 },
    Complete { message: String, timestamp: u64 },
    FileAnalysis { file: String, status: String, timestamp: u64 },
}

impl LogEvent {
    pub fn info(message: impl Into<String>) -> Self {
        Self::Info {
            message: message.into(),
            timestamp: current_timestamp(),
        }
    }

    pub fn progress(message: impl Into<String>, current: usize, total: usize) -> Self {
        Self::Progress {
            message: message.into(),
            current,
            total,
            timestamp: current_timestamp(),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self::Warning {
            message: message.into(),
            timestamp: current_timestamp(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
            timestamp: current_timestamp(),
        }
    }

    pub fn complete(message: impl Into<String>) -> Self {
        Self::Complete {
            message: message.into(),
            timestamp: current_timestamp(),
        }
    }

    pub fn file_analysis(file: impl Into<String>, status: impl Into<String>) -> Self {
        Self::FileAnalysis {
            file: file.into(),
            status: status.into(),
            timestamp: current_timestamp(),
        }
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

pub type LogSender = broadcast::Sender<LogEvent>;

pub struct WebSocketServer {
    tx: LogSender,
}

impl WebSocketServer {
    pub fn new(capacity: usize) -> (Self, LogSender) {
        let (tx, _) = broadcast::channel(capacity);
        let tx_clone = tx.clone();
        (Self { tx }, tx_clone)
    }

    pub async fn start(self, addr: SocketAddr) -> anyhow::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        println!("🌐 WebSocket server listening on ws://{}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    let tx = self.tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, tx, peer).await {
                            eprintln!("WebSocket connection error from {}: {}", peer, e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Failed to accept connection: {}", e);
                }
            }
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    tx: LogSender,
    peer: SocketAddr,
) -> anyhow::Result<()> {
    let ws_stream = accept_async(stream).await?;
    println!("✅ WebSocket client connected: {}", peer);

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let mut rx = tx.subscribe();

    // Send initial connection message
    let welcome = LogEvent::info("Connected to Sanctifier analysis stream");
    let msg = serde_json::to_string(&welcome)?;
    ws_sender.send(Message::Text(msg)).await?;

    // Spawn task to forward broadcast messages to this client
    let send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&event) {
                if ws_sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    });

    // Handle incoming messages (ping/pong)
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(data)) => {
                // Respond to ping with pong
                if ws_sender.send(Message::Pong(data)).await.is_err() {
                    break;
                }
            }
            Err(_) => break,
            _ => {}
        }
    }

    send_task.abort();
    println!("❌ WebSocket client disconnected: {}", peer);
    Ok(())
}
