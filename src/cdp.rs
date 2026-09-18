#![allow(clippy::result_large_err)]

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

#[allow(clippy::result_large_err)]
#[derive(Debug, thiserror::Error)]
pub enum CdpError {
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Protocol error ({code}): {message}")]
    ProtocolError { code: i64, message: String },
    #[error("Timeout awaiting response")]
    Timeout,
    #[error("Connection closed")]
    ConnectionClosed,
    #[error("Supervisor error: {0}")]
    SupervisorError(String),
}

/// Formats a Chrome DevTools Protocol JSON-RPC request frame.
pub fn format_cdp_request(id: u64, method: &str, params: Value, session_id: Option<&str>) -> Value {
    let mut req = serde_json::json!({
        "id": id,
        "method": method,
        "params": params,
    });
    if let Some(sid) = session_id {
        req["sessionId"] = Value::String(sid.to_string());
    }
    req
}

/// Parses a Chrome DevTools Protocol JSON-RPC response frame into a Result.
pub fn parse_cdp_response(val: Value) -> Result<Value, CdpError> {
    if !val.is_object() {
        return Err(CdpError::ProtocolError {
            code: -32600,
            message: "Invalid JSON-RPC format: not an object".into(),
        });
    }

    if let Some(err_val) = val.get("error") {
        let (code, message) = if let Some(err_obj) = err_val.as_object() {
            let code = err_obj.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
            let message = err_obj
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown protocol error")
                .to_string();
            (code, message)
        } else {
            (-1, err_val.to_string())
        };
        return Err(CdpError::ProtocolError { code, message });
    }

    if val.get("id").is_none() {
        return Err(CdpError::ProtocolError {
            code: -32600,
            message: "Invalid JSON-RPC response: missing id".into(),
        });
    }

    if let Some(result) = val.get("result") {
        Ok(result.clone())
    } else {
        Err(CdpError::ProtocolError {
            code: -32600,
            message: "Invalid JSON-RPC response: missing result or error".into(),
        })
    }
}

type PendingMap = Arc<std::sync::Mutex<HashMap<u64, oneshot::Sender<Result<Value, CdpError>>>>>;

/// An asynchronous WebSocket CDP client with concurrent message multiplexing.
#[derive(Clone)]
pub struct CdpClient {
    next_id: Arc<AtomicU64>,
    out_tx: mpsc::UnboundedSender<Message>,
    pending: PendingMap,
}

impl CdpClient {
    /// Connects to a WebSocket CDP endpoint and spawns the reader/writer tasks.
    pub async fn connect(ws_url: &str) -> Result<Self, CdpError> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(ws_url).await?;
        let (mut write_half, mut read_half) = ws_stream.split();
        let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Message>();
        let pending: PendingMap = Arc::new(std::sync::Mutex::new(HashMap::new()));

        // Writer loop
        tokio::spawn(async move {
            while let Some(msg) = out_rx.recv().await {
                if write_half.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // Reader loop
        let pending_clone = Arc::clone(&pending);
        tokio::spawn(async move {
            while let Some(msg_res) = read_half.next().await {
                match msg_res {
                    Ok(Message::Text(text)) => {
                        Self::dispatch_message(&pending_clone, &text);
                    }
                    Ok(Message::Binary(bin)) => {
                        if let Ok(text) = std::str::from_utf8(&bin) {
                            Self::dispatch_message(&pending_clone, text);
                        }
                    }
                    Ok(Message::Close(_)) => {
                        break;
                    }
                    Err(_) => {
                        break;
                    }
                    _ => {}
                }
            }

            // Connection terminated: notify all waiting callers
            let mut map = pending_clone.lock().unwrap();
            for (_, sender) in map.drain() {
                let _ = sender.send(Err(CdpError::ConnectionClosed));
            }
        });

        Ok(Self {
            next_id: Arc::new(AtomicU64::new(1)),
            out_tx,
            pending,
        })
    }

    fn dispatch_message(pending: &PendingMap, text: &str) {
        if let Ok(val) = serde_json::from_str::<Value>(text) {
            if let Some(id) = val.get("id").and_then(|i| i.as_u64()) {
                let sender_opt = {
                    let mut map = pending.lock().unwrap();
                    map.remove(&id)
                };
                if let Some(sender) = sender_opt {
                    let parsed = parse_cdp_response(val);
                    let _ = sender.send(parsed);
                }
            }
            // Event messages or unhandled notifications are ignored
        }
    }

    /// Sends a CDP request with a custom timeout.
    pub async fn send_request_timeout(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
        timeout: Duration,
    ) -> Result<Value, CdpError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        {
            let mut map = self.pending.lock().unwrap();
            map.insert(id, tx);
        }

        let req = format_cdp_request(id, method, params, session_id);
        let text = serde_json::to_string(&req)?;
        if self.out_tx.send(Message::Text(text.into())).is_err() {
            let mut map = self.pending.lock().unwrap();
            map.remove(&id);
            return Err(CdpError::ConnectionClosed);
        }

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => Err(CdpError::ConnectionClosed),
            Err(_) => {
                let mut map = self.pending.lock().unwrap();
                map.remove(&id);
                Err(CdpError::Timeout)
            }
        }
    }

    /// Sends a CDP request with a default 30-second timeout.
    pub async fn send_request(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, CdpError> {
        self.send_request_timeout(method, params, session_id, Duration::from_secs(30))
            .await
    }

    /// Alias for `send_request`.
    pub async fn call(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, CdpError> {
        self.send_request(method, params, session_id).await
    }
}

fn parse_host_port(cdp_url: Option<&str>) -> (String, u16) {
    let default_host = "127.0.0.1".to_string();
    let default_port = 9222;

    let Some(raw) = cdp_url else {
        return (default_host, default_port);
    };

    let stripped = raw
        .trim_start_matches("ws://")
        .trim_start_matches("http://")
        .trim_start_matches("https://");

    let authority = stripped.split('/').next().unwrap_or(stripped);

    if let Some((h, p)) = authority.split_once(':') {
        let port = p.parse::<u16>().unwrap_or(default_port);
        let host = if h.is_empty() {
            default_host
        } else {
            h.to_string()
        };
        (host, port)
    } else if !authority.is_empty() {
        (authority.to_string(), default_port)
    } else {
        (default_host, default_port)
    }
}

fn find_obscura_binary() -> Option<std::path::PathBuf> {
    if let Ok(bin) = std::env::var("OBSCURA_BIN") {
        let p = std::path::PathBuf::from(bin);
        if p.is_file() {
            return Some(p);
        }
    }

    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join("obscura");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

async fn resolve_ws_url(host: &str, port: u16, cdp_url: Option<&str>) -> Result<String, CdpError> {
    if let Some(url) = cdp_url {
        if url.starts_with("ws://") || url.starts_with("wss://") {
            return Ok(url.to_string());
        }
    }

    // Attempt to inspect /json/version endpoint for webSocketDebuggerUrl
    let http_url = format!("http://{}:{}/json/version", host, port);
    if let Ok(resp) = reqwest::Client::new()
        .get(&http_url)
        .timeout(Duration::from_millis(300))
        .send()
        .await
    {
        if let Ok(val) = resp.json::<Value>().await {
            if let Some(ws_url) = val.get("webSocketDebuggerUrl").and_then(|u| u.as_str()) {
                return Ok(ws_url.to_string());
            }
        }
    }

    Ok(format!("ws://{}:{}", host, port))
}

/// Ensures Obscura or a CDP-compatible endpoint is running and returns the WebSocket URL.
pub async fn ensure_obscura(cdp_url: Option<&str>) -> Result<String, CdpError> {
    let (host, port) = parse_host_port(cdp_url);
    let target_addr = format!("{}:{}", host, port);

    // 1. Probe TCP connection
    if tokio::net::TcpStream::connect(&target_addr).await.is_ok() {
        return resolve_ws_url(&host, port, cdp_url).await;
    }

    // 2. Unreachable: search for obscura binary
    let bin_path = find_obscura_binary().ok_or_else(|| {
        CdpError::SupervisorError(
            "Obscura browser binary not found. Please install obscura (https://obscura.sh/) or start via Docker: docker run -p 9222:9222 ghcr.io/obscura/obscura".to_string(),
        )
    })?;

    // 3. Spawn obscura serve in background
    let port_str = port.to_string();
    let mut cmd = tokio::process::Command::new(bin_path);
    cmd.args([
        "serve",
        "--port",
        &port_str,
        "--allow-file-access",
        "--allow-private-network",
    ])
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null());

    let _child = cmd
        .spawn()
        .map_err(|e| CdpError::SupervisorError(format!("Failed to spawn obscura: {}", e)))?;

    // 4. Poll readiness up to 3.0 seconds
    let timeout = Duration::from_secs_f64(3.0);
    let poll_interval = Duration::from_millis(50);
    let start = std::time::Instant::now();
    let mut connected = false;

    while start.elapsed() < timeout {
        if tokio::net::TcpStream::connect(&target_addr).await.is_ok() {
            connected = true;
            break;
        }
        tokio::time::sleep(poll_interval).await;
    }

    if !connected {
        return Err(CdpError::SupervisorError(format!(
            "Timed out waiting for Obscura to start on {}",
            target_addr
        )));
    }

    resolve_ws_url(&host, port, cdp_url).await
}
