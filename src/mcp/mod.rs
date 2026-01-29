//! MCP (Model Context Protocol) integration for clewdr.
//!
//! This module provides functionality to connect to MCP servers and use their
//! tools, resources, and prompts.

pub mod client;
pub mod registry;
pub mod router;
pub mod state;

pub use client::McpClient;
pub use registry::ToolRegistry;
pub use router::ToolRouter;
pub use state::{
    get_mcp_router, init_mcp_router, list_mcp_servers, list_mcp_tools, mcp_has_tools,
    reload_mcp_router, shutdown_mcp,
};
