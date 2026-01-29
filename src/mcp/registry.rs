//! Tool Registry for managing MCP tools from multiple servers.

use std::collections::HashMap;

use serde_json::Value;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::types::{
    claude::{CustomTool, Tool},
    mcp::{McpServerConfig, RegisteredTool},
};

use super::client::{McpClient, McpClientError};

/// Registry for MCP tools from multiple servers.
pub struct ToolRegistry {
    /// MCP clients indexed by server name.
    clients: RwLock<HashMap<String, McpClient>>,
    /// Registered tools indexed by qualified name (server::tool).
    tools: RwLock<HashMap<String, RegisteredTool>>,
    /// Tool name to qualified name mapping (for non-prefixed lookups).
    tool_aliases: RwLock<HashMap<String, String>>,
}

impl ToolRegistry {
    /// Create a new empty tool registry.
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
            tools: RwLock::new(HashMap::new()),
            tool_aliases: RwLock::new(HashMap::new()),
        }
    }

    /// Add an MCP server to the registry.
    pub async fn add_server(&self, config: McpServerConfig) -> Result<(), McpClientError> {
        if !config.enabled {
            info!("MCP server '{}' is disabled, skipping", config.name);
            return Ok(());
        }

        let server_name = config.name.clone();
        let mut client = McpClient::new(config);

        client.connect().await?;

        // Fetch and register tools
        match client.list_tools().await {
            Ok(tools) => {
                info!(
                    "MCP server '{}' provides {} tools",
                    server_name,
                    tools.len()
                );

                let mut registry_tools = self.tools.write().await;
                let mut aliases = self.tool_aliases.write().await;

                for tool in tools {
                    let registered = RegisteredTool::new(&server_name, tool);
                    debug!(
                        "Registered tool: {} from server '{}'",
                        registered.qualified_name, server_name
                    );

                    // Add alias for tool name (first server wins if conflict)
                    if !aliases.contains_key(&registered.tool.name) {
                        aliases.insert(
                            registered.tool.name.clone(),
                            registered.qualified_name.clone(),
                        );
                    } else {
                        warn!(
                            "Tool name '{}' already registered by another server, use qualified name '{}'",
                            registered.tool.name, registered.qualified_name
                        );
                    }

                    registry_tools.insert(registered.qualified_name.clone(), registered);
                }
            }
            Err(e) => {
                warn!("Failed to list tools from MCP server '{}': {}", server_name, e);
            }
        }

        // Store the client
        let mut clients = self.clients.write().await;
        clients.insert(server_name, client);

        Ok(())
    }

    /// Remove an MCP server from the registry.
    pub async fn remove_server(&self, name: &str) {
        let mut clients = self.clients.write().await;
        if let Some(mut client) = clients.remove(name) {
            client.disconnect().await;
        }

        // Remove tools from this server
        let mut tools = self.tools.write().await;
        let mut aliases = self.tool_aliases.write().await;

        tools.retain(|_, tool| tool.server_name != name);
        aliases.retain(|_, qualified| !qualified.starts_with(&format!("{}::", name)));

        info!("Removed MCP server '{}' from registry", name);
    }

    /// Get all registered tools as Claude API tool definitions.
    pub async fn get_claude_tools(&self) -> Vec<Tool> {
        let tools = self.tools.read().await;
        tools
            .values()
            .map(|t| {
                Tool::Custom(CustomTool {
                    name: t.qualified_name.clone(),
                    description: t.tool.description.clone(),
                    input_schema: t.tool.input_schema.clone(),
                    allowed_callers: None,
                    cache_control: None,
                    defer_loading: None,
                    input_examples: None,
                    strict: None,
                    type_: None,
                    extra: std::collections::HashMap::new(),
                })
            })
            .collect()
    }

    /// Get tools for a specific server.
    pub async fn get_server_tools(&self, server_name: &str) -> Vec<Tool> {
        let tools = self.tools.read().await;
        tools
            .values()
            .filter(|t| t.server_name == server_name)
            .map(|t| {
                Tool::Custom(CustomTool {
                    name: t.tool.name.clone(), // Use short name for server-specific tools
                    description: t.tool.description.clone(),
                    input_schema: t.tool.input_schema.clone(),
                    allowed_callers: None,
                    cache_control: None,
                    defer_loading: None,
                    input_examples: None,
                    strict: None,
                    type_: None,
                    extra: std::collections::HashMap::new(),
                })
            })
            .collect()
    }

    /// Look up a tool by name (supports both qualified and short names).
    pub async fn lookup_tool(&self, name: &str) -> Option<RegisteredTool> {
        let tools = self.tools.read().await;

        // Try qualified name first
        if let Some(tool) = tools.get(name) {
            return Some(tool.clone());
        }

        // Try alias lookup
        let aliases = self.tool_aliases.read().await;
        if let Some(qualified) = aliases.get(name) {
            return tools.get(qualified).cloned();
        }

        None
    }

    /// Call a tool by name.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Option<Value>,
    ) -> Result<Value, McpClientError> {
        // Look up the tool
        let tool_info = self.lookup_tool(name).await.ok_or_else(|| {
            McpClientError::InvalidResponse(format!("Tool '{}' not found", name))
        })?;

        // Get the client for this tool's server
        let mut clients = self.clients.write().await;
        let client = clients.get_mut(&tool_info.server_name).ok_or_else(|| {
            McpClientError::InvalidResponse(format!(
                "Server '{}' not connected",
                tool_info.server_name
            ))
        })?;

        // Call the tool using its original name (not qualified)
        let result = client.call_tool(&tool_info.tool.name, arguments).await?;

        // Convert MCP result to a simple value
        let content: Vec<Value> = result
            .content
            .into_iter()
            .map(|c| match c {
                crate::types::mcp::ToolContent::Text { text } => Value::String(text),
                crate::types::mcp::ToolContent::Image { data, mime_type } => {
                    serde_json::json!({
                        "type": "image",
                        "data": data,
                        "mime_type": mime_type
                    })
                }
                crate::types::mcp::ToolContent::Resource { resource } => {
                    serde_json::json!({
                        "type": "resource",
                        "uri": resource.uri,
                        "mime_type": resource.mime_type,
                        "text": resource.text,
                    })
                }
            })
            .collect();

        // Return single value if only one content, otherwise array
        if content.len() == 1 {
            Ok(content.into_iter().next().unwrap())
        } else {
            Ok(Value::Array(content))
        }
    }

    /// Check if a tool exists in the registry.
    pub async fn has_tool(&self, name: &str) -> bool {
        self.lookup_tool(name).await.is_some()
    }

    /// Get list of all tool names (qualified).
    pub async fn list_tool_names(&self) -> Vec<String> {
        let tools = self.tools.read().await;
        tools.keys().cloned().collect()
    }

    /// Get list of connected server names.
    pub async fn list_servers(&self) -> Vec<String> {
        let clients = self.clients.read().await;
        clients.keys().cloned().collect()
    }

    /// Refresh tools from all connected servers.
    pub async fn refresh_tools(&self) -> Result<(), McpClientError> {
        let server_names: Vec<String> = {
            let clients = self.clients.read().await;
            clients.keys().cloned().collect()
        };

        // Clear existing tools
        {
            let mut tools = self.tools.write().await;
            let mut aliases = self.tool_aliases.write().await;
            tools.clear();
            aliases.clear();
        }

        // Refresh from each server
        for server_name in server_names {
            let mut clients = self.clients.write().await;
            if let Some(client) = clients.get_mut(&server_name) {
                match client.list_tools().await {
                    Ok(tools) => {
                        let mut registry_tools = self.tools.write().await;
                        let mut aliases = self.tool_aliases.write().await;

                        for tool in tools {
                            let registered = RegisteredTool::new(&server_name, tool);

                            if !aliases.contains_key(&registered.tool.name) {
                                aliases.insert(
                                    registered.tool.name.clone(),
                                    registered.qualified_name.clone(),
                                );
                            }

                            registry_tools.insert(registered.qualified_name.clone(), registered);
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to refresh tools from server '{}': {}",
                            server_name, e
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Disconnect all servers and clear the registry.
    pub async fn shutdown(&self) {
        let mut clients = self.clients.write().await;
        for (name, mut client) in clients.drain() {
            info!("Disconnecting MCP server '{}'", name);
            client.disconnect().await;
        }

        let mut tools = self.tools.write().await;
        let mut aliases = self.tool_aliases.write().await;
        tools.clear();
        aliases.clear();
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_creation() {
        let registry = ToolRegistry::new();
        assert!(registry.list_servers().await.is_empty());
        assert!(registry.list_tool_names().await.is_empty());
    }
}
