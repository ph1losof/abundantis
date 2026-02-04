//! Stdio JSON-RPC transport layer.
//!
//! This module implements bidirectional JSON-RPC 2.0 communication over stdio
//! (stdin/stdout) for external provider processes.
//!
//! ## Message Framing
//!
//! Messages are framed using the LSP content-length header format:
//! ```text
//! Content-Length: <bytes>\r\n
//! \r\n
//! <json content>
//! ```
//!
//! ## Thread Model
//!
//! - Writer: Single async task writes to stdin
//! - Reader: Single async task reads from stdout
//! - Pending requests tracked with channels for response delivery

use super::protocol::{JsonRpcId, JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use crate::error::SourceError;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;
use tokio::sync::{mpsc, oneshot};

/// Transport error type.
#[derive(Debug, Clone)]
pub enum TransportError {
    /// Process not running.
    NotRunning,
    /// Failed to send message.
    SendFailed(String),
    /// Failed to receive response.
    ReceiveFailed(String),
    /// Response timeout.
    Timeout,
    /// Process exited unexpectedly.
    ProcessExited(Option<i32>),
    /// Parse error.
    ParseError(String),
    /// IO error.
    IoError(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::NotRunning => write!(f, "Provider process not running"),
            TransportError::SendFailed(msg) => write!(f, "Failed to send: {}", msg),
            TransportError::ReceiveFailed(msg) => write!(f, "Failed to receive: {}", msg),
            TransportError::Timeout => write!(f, "Request timed out"),
            TransportError::ProcessExited(code) => {
                if let Some(c) = code {
                    write!(f, "Process exited with code {}", c)
                } else {
                    write!(f, "Process exited")
                }
            }
            TransportError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            TransportError::IoError(msg) => write!(f, "IO error: {}", msg),
        }
    }
}

impl std::error::Error for TransportError {}

impl From<TransportError> for SourceError {
    fn from(e: TransportError) -> Self {
        SourceError::Connection {
            provider: "external".into(),
            reason: e.to_string(),
        }
    }
}

/// A pending request waiting for a response.
struct PendingRequest {
    sender: oneshot::Sender<Result<JsonRpcResponse, TransportError>>,
}

/// Stdio transport for JSON-RPC communication with a child process.
pub struct StdioTransport {
    /// Next request ID.
    next_id: AtomicI64,
    /// Pending requests awaiting responses.
    pending: Arc<Mutex<HashMap<JsonRpcId, PendingRequest>>>,
    /// Channel to send messages to the writer task.
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    /// Handle to the reader task.
    reader_handle: Option<tokio::task::JoinHandle<()>>,
    /// Handle to the writer task.
    writer_handle: Option<tokio::task::JoinHandle<()>>,
    /// Channel for incoming notifications.
    notification_rx: mpsc::UnboundedReceiver<JsonRpcNotification>,
    /// Whether the transport is running.
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl StdioTransport {
    /// Creates a new transport from a child process.
    ///
    /// The child process must have stdin and stdout piped.
    pub fn new(child: &mut Child) -> Result<Self, TransportError> {
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| TransportError::IoError("No stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TransportError::IoError("No stdout".into()))?;

        let pending: Arc<Mutex<HashMap<JsonRpcId, PendingRequest>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));

        // Channel for notifications
        let (notification_tx, notification_rx) = mpsc::unbounded_channel();

        // Channel for write requests
        let (write_tx, write_rx) = mpsc::unbounded_channel();

        // Spawn writer task
        let writer_handle = {
            let running = Arc::clone(&running);
            tokio::spawn(async move {
                writer_task(stdin, write_rx, running).await;
            })
        };

        // Spawn reader task
        let reader_handle = {
            let pending = Arc::clone(&pending);
            let running = Arc::clone(&running);
            tokio::spawn(async move {
                reader_task(stdout, pending, notification_tx, running).await;
            })
        };

        Ok(Self {
            next_id: AtomicI64::new(1),
            pending,
            write_tx,
            reader_handle: Some(reader_handle),
            writer_handle: Some(writer_handle),
            notification_rx,
            running,
        })
    }

    /// Sends a request and waits for the response.
    pub async fn request<T: serde::Serialize>(
        &self,
        method: &str,
        params: T,
    ) -> Result<JsonRpcResponse, TransportError> {
        self.request_with_timeout(method, params, std::time::Duration::from_secs(30))
            .await
    }

    /// Sends a request with a custom timeout.
    pub async fn request_with_timeout<T: serde::Serialize>(
        &self,
        method: &str,
        params: T,
        timeout: std::time::Duration,
    ) -> Result<JsonRpcResponse, TransportError> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(TransportError::NotRunning);
        }

        let id = JsonRpcId::Number(self.next_id.fetch_add(1, Ordering::SeqCst));
        let request = JsonRpcRequest::new(id.clone(), method).with_params(params);

        // Create response channel
        let (tx, rx) = oneshot::channel();

        // Register pending request
        {
            let mut pending = self.pending.lock();
            pending.insert(id.clone(), PendingRequest { sender: tx });
        }

        // Serialize and send
        let json = serde_json::to_string(&request)
            .map_err(|e| TransportError::SendFailed(e.to_string()))?;
        let message = format_message(&json);

        self.write_tx
            .send(message)
            .map_err(|_| TransportError::SendFailed("Writer channel closed".into()))?;

        // Wait for response with timeout
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                // Channel dropped, remove from pending
                self.pending.lock().remove(&id);
                Err(TransportError::ReceiveFailed("Response channel dropped".into()))
            }
            Err(_) => {
                // Timeout, remove from pending
                self.pending.lock().remove(&id);
                Err(TransportError::Timeout)
            }
        }
    }

    /// Sends a notification (no response expected).
    pub fn notify<T: serde::Serialize>(&self, method: &str, params: T) -> Result<(), TransportError> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(TransportError::NotRunning);
        }

        let notification = JsonRpcNotification::new(method).with_params(params);
        let json = serde_json::to_string(&notification)
            .map_err(|e| TransportError::SendFailed(e.to_string()))?;
        let message = format_message(&json);

        self.write_tx
            .send(message)
            .map_err(|_| TransportError::SendFailed("Writer channel closed".into()))?;

        Ok(())
    }

    /// Receives the next notification.
    ///
    /// Returns `None` if the transport is shutting down.
    pub async fn recv_notification(&mut self) -> Option<JsonRpcNotification> {
        self.notification_rx.recv().await
    }

    /// Checks if the transport is still running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Shuts down the transport.
    pub async fn shutdown(&mut self) {
        self.running.store(false, Ordering::SeqCst);

        // Cancel all pending requests
        {
            let mut pending = self.pending.lock();
            for (_, req) in pending.drain() {
                let _ = req.sender.send(Err(TransportError::NotRunning));
            }
        }

        // Wait for tasks to complete
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.await;
        }
        if let Some(handle) = self.writer_handle.take() {
            let _ = handle.await;
        }
    }
}

/// Formats a JSON message with content-length header.
fn format_message(json: &str) -> Vec<u8> {
    let len = json.len();
    format!("Content-Length: {}\r\n\r\n{}", len, json).into_bytes()
}

/// Writer task that sends messages to stdin.
async fn writer_task(
    mut stdin: ChildStdin,
    mut rx: mpsc::UnboundedReceiver<Vec<u8>>,
    running: Arc<std::sync::atomic::AtomicBool>,
) {
    while running.load(Ordering::SeqCst) {
        match rx.recv().await {
            Some(message) => {
                if let Err(e) = stdin.write_all(&message) {
                    tracing::error!("Failed to write to stdin: {}", e);
                    break;
                }
                if let Err(e) = stdin.flush() {
                    tracing::error!("Failed to flush stdin: {}", e);
                    break;
                }
            }
            None => break,
        }
    }
}

/// Reader task that reads messages from stdout.
async fn reader_task(
    stdout: ChildStdout,
    pending: Arc<Mutex<HashMap<JsonRpcId, PendingRequest>>>,
    notification_tx: mpsc::UnboundedSender<JsonRpcNotification>,
    running: Arc<std::sync::atomic::AtomicBool>,
) {
    // Use blocking IO in a spawn_blocking context
    let result = tokio::task::spawn_blocking(move || {
        let mut reader = BufReader::new(stdout);

        while running.load(Ordering::SeqCst) {
            match read_message(&mut reader) {
                Ok(Some(message)) => {
                    match message {
                        JsonRpcMessage::Response(response) => {
                            // Find and complete the pending request
                            let mut pending_guard = pending.lock();
                            if let Some(req) = pending_guard.remove(&response.id) {
                                let _ = req.sender.send(Ok(response));
                            }
                        }
                        JsonRpcMessage::Notification(notification) => {
                            let _ = notification_tx.send(notification);
                        }
                        JsonRpcMessage::Request(request) => {
                            // Providers shouldn't send requests to the client
                            tracing::warn!("Unexpected request from provider: {}", request.method);
                        }
                    }
                }
                Ok(None) => {
                    // EOF
                    tracing::debug!("Provider stdout closed");
                    break;
                }
                Err(e) => {
                    tracing::error!("Failed to read from stdout: {}", e);
                    break;
                }
            }
        }
    });

    let _ = result.await;
}

/// Reads a single JSON-RPC message from a buffered reader.
fn read_message(reader: &mut BufReader<ChildStdout>) -> Result<Option<JsonRpcMessage>, TransportError> {
    // Read headers
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => return Ok(None), // EOF
            Ok(_) => {
                let line = line.trim_end();
                if line.is_empty() {
                    break; // End of headers
                }
                if let Some(len_str) = line.strip_prefix("Content-Length:") {
                    content_length = len_str.trim().parse().ok();
                }
                // Ignore other headers (Content-Type, etc.)
            }
            Err(e) => {
                return Err(TransportError::IoError(e.to_string()));
            }
        }
    }

    let content_length = content_length
        .ok_or_else(|| TransportError::ParseError("Missing Content-Length header".into()))?;

    // Read content
    let mut content = vec![0u8; content_length];
    reader
        .read_exact(&mut content)
        .map_err(|e| TransportError::IoError(e.to_string()))?;

    let content_str = String::from_utf8(content)
        .map_err(|e| TransportError::ParseError(e.to_string()))?;

    // Parse message
    let message: JsonRpcMessage = serde_json::from_str(&content_str)
        .map_err(|e| TransportError::ParseError(e.to_string()))?;

    Ok(Some(message))
}

/// Synchronous transport for simple request-response patterns.
///
/// Use this for initial setup before async runtime is available.
pub struct SyncStdioTransport {
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    next_id: i64,
}

impl SyncStdioTransport {
    /// Creates a new synchronous transport.
    pub fn new(child: &mut Child) -> Result<Self, TransportError> {
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| TransportError::IoError("No stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TransportError::IoError("No stdout".into()))?;

        Ok(Self {
            stdin,
            reader: BufReader::new(stdout),
            next_id: 1,
        })
    }

    /// Sends a request and waits for the response.
    pub fn request<T: serde::Serialize>(
        &mut self,
        method: &str,
        params: T,
    ) -> Result<JsonRpcResponse, TransportError> {
        let id = JsonRpcId::Number(self.next_id);
        self.next_id += 1;

        let request = JsonRpcRequest::new(id.clone(), method).with_params(params);
        let json = serde_json::to_string(&request)
            .map_err(|e| TransportError::SendFailed(e.to_string()))?;

        // Send
        let message = format_message(&json);
        self.stdin
            .write_all(&message)
            .map_err(|e| TransportError::IoError(e.to_string()))?;
        self.stdin
            .flush()
            .map_err(|e| TransportError::IoError(e.to_string()))?;

        // Wait for response
        loop {
            match read_message(&mut self.reader)? {
                Some(JsonRpcMessage::Response(response)) => {
                    if response.id == id {
                        return Ok(response);
                    }
                    // Not our response, continue waiting
                }
                Some(JsonRpcMessage::Notification(_)) => {
                    // Ignore notifications during sync request
                    continue;
                }
                Some(JsonRpcMessage::Request(_)) => {
                    // Ignore requests
                    continue;
                }
                None => {
                    return Err(TransportError::ProcessExited(None));
                }
            }
        }
    }

    /// Sends a notification.
    pub fn notify<T: serde::Serialize>(&mut self, method: &str, params: T) -> Result<(), TransportError> {
        let notification = JsonRpcNotification::new(method).with_params(params);
        let json = serde_json::to_string(&notification)
            .map_err(|e| TransportError::SendFailed(e.to_string()))?;
        let message = format_message(&json);

        self.stdin
            .write_all(&message)
            .map_err(|e| TransportError::IoError(e.to_string()))?;
        self.stdin
            .flush()
            .map_err(|e| TransportError::IoError(e.to_string()))?;

        Ok(())
    }
}

/// Spawns a provider process with proper stdio configuration.
pub fn spawn_provider(binary_path: &std::path::Path) -> Result<Child, TransportError> {
    std::process::Command::new(binary_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| TransportError::IoError(format!("Failed to spawn provider: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_message() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let message = format_message(json);
        let message_str = String::from_utf8(message).unwrap();

        assert!(message_str.starts_with("Content-Length: 40\r\n\r\n"));
        assert!(message_str.ends_with(json));
    }

    #[test]
    fn test_transport_error_display() {
        let error = TransportError::Timeout;
        assert_eq!(error.to_string(), "Request timed out");

        let error = TransportError::ProcessExited(Some(1));
        assert_eq!(error.to_string(), "Process exited with code 1");
    }
}
