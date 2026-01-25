//! MCP Client implementation for stdio and HTTP transport.

use std::{
    collections::HashMap,
    process::Stdio,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc,
    },
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    sync::{Mutex, oneshot},
    time::{Duration, timeout},
};
use tracing::{debug, error, info};

use crate::types::mcp::{
    CallToolParams, CallToolResult, InitializeParams, InitializeResult, ListToolsResult,
    McpError, McpRequest, McpResponse, McpServerConfig, McpTool, RequestId, ServerCapabilities,
};

/// Error type for MCP client operations.
#[derive(Debug, thiserror::Error)]
pub enum McpClientError {
    #[error("Failed to spawn process: {0}")]
    SpawnError(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Server returned error: {0}")]
    ServerError(McpError),
    #[error("Timeout waiting for response")]
    Timeout,
    #[error("Connection closed")]
    ConnectionClosed,
    #[error("Request cancelled")]
    Cancelled,
    #[error("Server not initialized")]
    NotInitialized,
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

/// Client for communicating with an MCP server.
pub struct McpClient {
    config: McpServerConfig,
    child: Option<Child>,
    stdin: Option<tokio::process::ChildStdin>,
    pending_requests: Arc<Mutex<HashMap<RequestId, oneshot::Sender<Result<Value, McpError>>>>>,
    next_id: AtomicI64,
    capabilities: Option<ServerCapabilities>,
    initialized: bool,
}

impl McpClient {
    /// Create a new MCP client with the given configuration.
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            child: None,
            stdin: None,
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicI64::new(1),
            capabilities: None,
            initialized: false,
        }
    }

    /// Get the server name.
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Check if the client is connected and initialized.
    pub fn is_ready(&self) -> bool {
        self.initialized && self.child.is_some()
    }

    /// Get server capabilities.
    pub fn capabilities(&self) -> Option<&ServerCapabilities> {
        self.capabilities.as_ref()
    }

    /// Connect to the MCP server and initialize the connection.
    pub async fn connect(&mut self) -> Result<(), McpClientError> {
        if self.child.is_some() {
            return Ok(());
        }

        let command = match &self.config.command {
            Some(cmd) => cmd.clone(),
            None => {
                return Err(McpClientError::InvalidResponse(
                    "No command specified for stdio transport".to_string(),
                ));
            }
        };

        info!("Connecting to MCP server '{}' via command: {}", self.config.name, command);

        let mut cmd = Command::new(&command);
        cmd.args(&self.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Set environment variables
        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| {
            McpClientError::InvalidResponse("Failed to get stdin".to_string())
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            McpClientError::InvalidResponse("Failed to get stdout".to_string())
        })?;

        self.child = Some(child);
        self.stdin = Some(stdin);

        // Start response reader task
        let pending = Arc::clone(&self.pending_requests);
        let server_name = self.config.name.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        debug!("MCP server '{}' closed stdout", server_name);
                        break;
                    }
                    Ok(_) => {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }

                        debug!("MCP '{}' received: {}", server_name, line);

                        match serde_json::from_str::<McpResponse>(line) {
                            Ok(response) => {
                                let mut pending = pending.lock().await;
                                if let Some(sender) = pending.remove(&response.id) {
                                    let result = if let Some(error) = response.error {
                                        Err(error)
                                    } else {
                                        Ok(response.result.unwrap_or(Value::Null))
                                    };
                                    let _ = sender.send(result);
                                }
                            }
                            Err(e) => {
                                // Could be a notification, just log it
                                debug!("Failed to parse MCP response: {} - line: {}", e, line);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error reading from MCP server '{}': {}", server_name, e);
                        break;
                    }
                }
            }
        });

        // Initialize the connection
        self.initialize().await?;

        Ok(())
    }

    /// Initialize the MCP connection.
    async fn initialize(&mut self) -> Result<(), McpClientError> {
        let params = InitializeParams::default();
        let result: InitializeResult = self
            .request("initialize", Some(serde_json::to_value(&params)?))
            .await?;

        info!(
            "MCP server '{}' initialized: {} v{} (protocol: {})",
            self.config.name,
            result.server_info.name,
            result.server_info.version,
            result.protocol_version
        );

        self.capabilities = Some(result.capabilities);

        // Send initialized notification
        self.notify("notifications/initialized", None).await?;

        self.initialized = true;
        Ok(())
    }

    /// Send a request to the MCP server and wait for response.
    pub async fn request<T: serde::de::DeserializeOwned>(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<T, McpClientError> {
        if !self.initialized && method != "initialize" {
            return Err(McpClientError::NotInitialized);
        }

        let id = RequestId::Number(self.next_id.fetch_add(1, Ordering::SeqCst));
        let request = McpRequest::new(id.clone(), method, params);

        let (tx, rx) = oneshot::channel();

        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(id.clone(), tx);
        }

        // Send request
        let stdin = self.stdin.as_mut().ok_or(McpClientError::ConnectionClosed)?;
        let json = serde_json::to_string(&request)?;
        debug!("MCP '{}' sending: {}", self.config.name, json);
        stdin.write_all(json.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;

        // Wait for response with timeout
        let timeout_duration = Duration::from_millis(self.config.timeout_ms);
        match timeout(timeout_duration, rx).await {
            Ok(Ok(Ok(value))) => {
                serde_json::from_value(value).map_err(McpClientError::JsonError)
            }
            Ok(Ok(Err(mcp_error))) => Err(McpClientError::ServerError(mcp_error)),
            Ok(Err(_)) => Err(McpClientError::Cancelled),
            Err(_) => {
                // Remove pending request on timeout
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&id);
                Err(McpClientError::Timeout)
            }
        }
    }

    /// Send a notification to the MCP server (no response expected).
    pub async fn notify(&mut self, method: &str, params: Option<Value>) -> Result<(), McpClientError> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        let stdin = self.stdin.as_mut().ok_or(McpClientError::ConnectionClosed)?;
        let json = serde_json::to_string(&notification)?;
        debug!("MCP '{}' notifying: {}", self.config.name, json);
        stdin.write_all(json.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;

        Ok(())
    }

    /// List available tools from the server.
    pub async fn list_tools(&mut self) -> Result<Vec<McpTool>, McpClientError> {
        let result: ListToolsResult = self.request("tools/list", None).await?;
        Ok(result.tools)
    }

    /// Call a tool on the server.
    pub async fn call_tool(
        &mut self,
        name: &str,
        arguments: Option<Value>,
    ) -> Result<CallToolResult, McpClientError> {
        let params = CallToolParams {
            name: name.to_string(),
            arguments,
        };

        self.request("tools/call", Some(serde_json::to_value(&params)?))
            .await
    }

    /// Disconnect from the MCP server.
    pub async fn disconnect(&mut self) {
        if let Some(mut child) = self.child.take() {
            // Try to kill gracefully first - ignore result
            if let Err(e) = child.kill().await {
                debug!("Failed to kill MCP server process: {}", e);
            }
        }
        self.stdin = None;
        self.initialized = false;
        self.capabilities = None;
        info!("Disconnected from MCP server '{}'", self.config.name);
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // Attempt to kill the child process if still running
        if let Some(mut child) = self.child.take() {
            // Use blocking kill since we're in drop - ignore result
            if child.start_kill().is_err() {
                // Process already terminated or other error
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let config = McpServerConfig {
            name: "test".to_string(),
            command: Some("echo".to_string()),
            args: vec!["hello".to_string()],
            ..Default::default()
        };

        let client = McpClient::new(config);
        assert_eq!(client.name(), "test");
        assert!(!client.is_ready());
    }
}
