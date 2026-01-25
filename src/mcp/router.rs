//! Tool Router for intercepting and routing tool calls.
//!
//! The router sits between Claude API responses and tool execution,
//! determining whether to execute tools locally via MCP or pass through.

use serde_json::Value;
use tracing::{debug, warn};

use super::registry::ToolRegistry;
use crate::types::claude::ContentBlock;

/// Routing decision for a tool call.
#[derive(Debug, Clone)]
pub enum ToolRoute {
    /// Execute via an MCP server.
    Mcp { server: String, tool: String },
    /// Pass through to the client (not handled by clewdr).
    PassThrough,
}

/// Tool router that determines how to handle tool calls.
pub struct ToolRouter {
    registry: ToolRegistry,
    /// Whether to enable MCP routing.
    enabled: bool,
}

impl ToolRouter {
    /// Create a new tool router with the given registry.
    pub fn new(registry: ToolRegistry) -> Self {
        Self {
            registry,
            enabled: true,
        }
    }

    /// Create a disabled tool router (all tools pass through).
    pub fn disabled() -> Self {
        Self {
            registry: ToolRegistry::new(),
            enabled: false,
        }
    }

    /// Get a reference to the tool registry.
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// Get a mutable reference to the tool registry.
    pub fn registry_mut(&mut self) -> &mut ToolRegistry {
        &mut self.registry
    }

    /// Enable or disable MCP routing.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if MCP routing is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Determine the route for a tool call.
    pub async fn route(&self, tool_name: &str) -> ToolRoute {
        if !self.enabled {
            return ToolRoute::PassThrough;
        }

        if let Some(tool) = self.registry.lookup_tool(tool_name).await {
            debug!("Routing tool '{}' to MCP server '{}'", tool_name, tool.server_name);
            ToolRoute::Mcp {
                server: tool.server_name,
                tool: tool.tool.name,
            }
        } else {
            debug!("Tool '{}' not found in MCP registry, passing through", tool_name);
            ToolRoute::PassThrough
        }
    }

    /// Execute a tool call if it's routed to MCP, otherwise return None.
    pub async fn execute_if_mcp(
        &self,
        tool_name: &str,
        arguments: Option<Value>,
    ) -> Option<Result<Value, String>> {
        if !self.enabled {
            return None;
        }

        match self.route(tool_name).await {
            ToolRoute::Mcp { .. } => {
                Some(
                    self.registry
                        .call_tool(tool_name, arguments)
                        .await
                        .map_err(|e| e.to_string()),
                )
            }
            ToolRoute::PassThrough => None,
        }
    }

    /// Process tool_use content blocks and execute MCP tools.
    /// Returns a list of tool results for MCP-handled tools.
    pub async fn process_tool_uses(
        &self,
        content_blocks: &[ContentBlock],
    ) -> Vec<ContentBlock> {
        if !self.enabled {
            return Vec::new();
        }

        let mut results = Vec::new();

        for block in content_blocks {
            if let ContentBlock::ToolUse { id, name, input } = block {
                if let Some(exec_result) = self.execute_if_mcp(name, Some(input.clone())).await {
                    let content = match exec_result {
                        Ok(value) => value,
                        Err(e) => {
                            warn!("MCP tool '{}' execution failed: {}", name, e);
                            Value::String(format!("Error: {}", e))
                        }
                    };

                    results.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content,
                    });
                }
            }
        }

        results
    }

    /// Check if any of the given tools are MCP tools.
    pub async fn has_mcp_tools(&self, tool_names: &[&str]) -> bool {
        if !self.enabled {
            return false;
        }

        for name in tool_names {
            if self.registry.has_tool(name).await {
                return true;
            }
        }
        false
    }

    /// Get all available MCP tools as Claude tool definitions.
    pub async fn get_mcp_tools(&self) -> Vec<crate::types::claude::Tool> {
        if !self.enabled {
            return Vec::new();
        }

        self.registry.get_claude_tools().await
    }

    /// Shutdown the router and disconnect all MCP servers.
    pub async fn shutdown(&self) {
        self.registry.shutdown().await;
    }
}

impl Default for ToolRouter {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Builder for creating a ToolRouter with MCP servers.
pub struct ToolRouterBuilder {
    servers: Vec<crate::types::mcp::McpServerConfig>,
    enabled: bool,
}

impl ToolRouterBuilder {
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
            enabled: true,
        }
    }

    /// Add an MCP server configuration.
    pub fn add_server(mut self, config: crate::types::mcp::McpServerConfig) -> Self {
        self.servers.push(config);
        self
    }

    /// Set whether MCP routing is enabled.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Build the ToolRouter, connecting to all configured servers.
    pub async fn build(self) -> ToolRouter {
        let registry = ToolRegistry::new();

        for server_config in self.servers {
            if let Err(e) = registry.add_server(server_config.clone()).await {
                warn!(
                    "Failed to connect to MCP server '{}': {}",
                    server_config.name, e
                );
            }
        }

        ToolRouter {
            registry,
            enabled: self.enabled,
        }
    }
}

impl Default for ToolRouterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_disabled_router() {
        let router = ToolRouter::disabled();
        assert!(!router.is_enabled());

        let route = router.route("any_tool").await;
        assert!(matches!(route, ToolRoute::PassThrough));
    }

    #[tokio::test]
    async fn test_passthrough_for_unknown_tool() {
        let router = ToolRouter::new(ToolRegistry::new());
        let route = router.route("unknown_tool").await;
        assert!(matches!(route, ToolRoute::PassThrough));
    }
}
