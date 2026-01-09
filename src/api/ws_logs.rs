use std::{
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use axum::{
    extract::{
        Query,
        WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::Response,
};
use futures::{SinkExt, StreamExt};
use notify::{Event, RecursiveMode, Watcher, recommended_watcher};
use serde::Deserialize;
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncSeekExt, BufReader, SeekFrom},
    sync::mpsc,
    time::interval,
};
use tracing::{error, info};

use crate::config::{CLEWDR_CONFIG, LOG_DIR};

/// Query parameters for WebSocket logs connection
#[derive(Deserialize)]
pub struct WsLogsQuery {
    /// Auth token for authentication
    token: String,
    /// Number of initial lines to send (default 100)
    #[serde(default = "default_initial_lines")]
    initial_lines: usize,
}

const fn default_initial_lines() -> usize {
    100
}

/// WebSocket endpoint for streaming logs in real-time
pub async fn api_ws_logs(
    ws: WebSocketUpgrade,
    Query(query): Query<WsLogsQuery>,
) -> Response {
    // Verify authentication
    if !CLEWDR_CONFIG.load().admin_auth(&query.token) {
        return Response::builder()
            .status(401)
            .body("Unauthorized".into())
            .unwrap();
    }

    ws.on_upgrade(move |socket| handle_log_stream(socket, query.initial_lines))
}

async fn handle_log_stream(socket: WebSocket, initial_lines: usize) {
    let (mut sender, mut receiver) = socket.split();

    // Find the current log file
    let log_dir = LOG_DIR.as_path();
    if !log_dir.exists() {
        let _ = sender
            .send(Message::Text(
                r#"{"type":"error","message":"Log directory does not exist"}"#.into(),
            ))
            .await;
        return;
    }

    // Get the most recent log file
    let log_file = match get_current_log_file(log_dir) {
        Some(path) => path,
        None => {
            let _ = sender
                .send(Message::Text(
                    r#"{"type":"error","message":"No log files found"}"#.into(),
                ))
                .await;
            return;
        }
    };

    info!("WebSocket log stream started for {:?}", log_file);

    // Send initial lines
    if let Err(e) = send_initial_lines(&mut sender, &log_file, initial_lines).await {
        error!("Failed to send initial lines: {}", e);
        return;
    }

    // Set up file watcher
    let (tx, mut rx) = mpsc::channel::<()>(32);
    let log_file_clone = log_file.clone();

    // Create a file system watcher
    let tx_clone = tx.clone();
    let watcher_result = recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            if event.kind.is_modify() || event.kind.is_create() {
                let _ = tx_clone.try_send(());
            }
        }
    });

    let mut watcher = match watcher_result {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to create file watcher: {}", e);
            // Fall back to polling mode
            let _ = sender
                .send(Message::Text(
                    r#"{"type":"info","message":"Using polling mode for log updates"}"#.into(),
                ))
                .await;
            poll_log_file(sender, receiver, log_file).await;
            return;
        }
    };

    // Watch the log directory
    if let Err(e) = watcher.watch(log_dir, RecursiveMode::NonRecursive) {
        error!("Failed to watch log directory: {}", e);
        poll_log_file(sender, receiver, log_file).await;
        return;
    }

    // Track file position
    let file = match File::open(&log_file_clone).await {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to open log file: {}", e);
            return;
        }
    };
    let mut reader = BufReader::new(file);
    // Seek to end
    if let Err(e) = reader.seek(SeekFrom::End(0)).await {
        error!("Failed to seek to end of log file: {}", e);
        return;
    }

    let sender = Arc::new(tokio::sync::Mutex::new(sender));
    let sender_clone = sender.clone();

    // Handle incoming WebSocket messages (for ping/pong and close)
    let recv_handle = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Close(_)) => break,
                Ok(Message::Ping(data)) => {
                    let mut s = sender_clone.lock().await;
                    let _ = s.send(Message::Pong(data)).await;
                }
                Err(_) => break,
                _ => {}
            }
        }
    });

    // Main loop: watch for file changes and send new lines
    loop {
        tokio::select! {
            _ = rx.recv() => {
                // File changed, read new lines
                let mut line = String::new();
                while let Ok(n) = reader.read_line(&mut line).await {
                    if n == 0 {
                        break;
                    }
                    let trimmed = line.trim_end();
                    if !trimmed.is_empty() {
                        let json = serde_json::json!({
                            "type": "log",
                            "line": trimmed
                        });
                        let mut s = sender.lock().await;
                        if s.send(Message::Text(json.to_string().into())).await.is_err() {
                            info!("WebSocket client disconnected");
                            recv_handle.abort();
                            return;
                        }
                    }
                    line.clear();
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                // Send heartbeat
                let mut s = sender.lock().await;
                if s.send(Message::Ping(vec![].into())).await.is_err() {
                    info!("WebSocket client disconnected (heartbeat failed)");
                    recv_handle.abort();
                    return;
                }
            }
        }
    }
}

/// Fallback polling mode when file watching is not available
async fn poll_log_file(
    mut sender: futures::stream::SplitSink<WebSocket, Message>,
    mut receiver: futures::stream::SplitStream<WebSocket>,
    log_file: PathBuf,
) {
    let file = match File::open(&log_file).await {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to open log file: {}", e);
            return;
        }
    };
    let mut reader = BufReader::new(file);
    if let Err(e) = reader.seek(SeekFrom::End(0)).await {
        error!("Failed to seek to end of log file: {}", e);
        return;
    }

    let mut poll_interval = interval(Duration::from_millis(500));

    loop {
        tokio::select! {
            _ = poll_interval.tick() => {
                let mut line = String::new();
                while let Ok(n) = reader.read_line(&mut line).await {
                    if n == 0 {
                        break;
                    }
                    let trimmed = line.trim_end();
                    if !trimmed.is_empty() {
                        let json = serde_json::json!({
                            "type": "log",
                            "line": trimmed
                        });
                        if sender.send(Message::Text(json.to_string().into())).await.is_err() {
                            info!("WebSocket client disconnected");
                            return;
                        }
                    }
                    line.clear();
                }
            }
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        let _ = sender.send(Message::Pong(data)).await;
                    }
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
}

fn get_current_log_file(log_dir: &std::path::Path) -> Option<PathBuf> {
    std::fs::read_dir(log_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("clewdr.log"))
        })
        .max_by_key(|e| e.path())
        .map(|e| e.path())
}

async fn send_initial_lines(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    log_file: &PathBuf,
    max_lines: usize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let content = tokio::fs::read_to_string(log_file).await?;
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(max_lines);

    for line in &lines[start..] {
        if !line.is_empty() {
            let json = serde_json::json!({
                "type": "log",
                "line": line
            });
            sender
                .send(Message::Text(json.to_string().into()))
                .await?;
        }
    }

    // Send marker that initial lines are done
    let json = serde_json::json!({
        "type": "init_complete",
        "count": lines[start..].len()
    });
    sender.send(Message::Text(json.to_string().into())).await?;

    Ok(())
}
