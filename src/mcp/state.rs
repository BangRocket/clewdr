//! Global MCP state management.

use arc_swap::ArcSwap;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::{info, warn};

use crate::config::CLEWDR_CONFIG;
use crate::types::mcp::McpServerConfig;

use super::router::{ToolRouter, ToolRouterBuilder};

/// Global MCP tool router instance.
static MCP_ROUTER: OnceCell<ArcSwap<ToolRouter>> = OnceCell::const_new();

/// Initialize the global MCP router with servers from configuration.
pub async fn init_mcp_router() {
    let config = CLEWDR_CONFIG.load();

    let router = if config.mcp_enabled && !config.mcp_servers.is_empty() {
        info!("Initializing MCP with {} configured servers", config.mcp_servers.len());

        let mut builder = ToolRouterBuilder::new().enabled(true);
        for server_config in config.mcp_servers.iter() {
            builder = builder.add_server(server_config.clone());
        }
        builder.build().await
    } else {
        if !config.mcp_enabled {
            info!("MCP is disabled in configuration");
        } else {
            info!("No MCP servers configured");
        }
        ToolRouter::disabled()
    };

    MCP_ROUTER
        .get_or_init(|| async { ArcSwap::from_pointee(router) })
        .await;
}

/// Get a reference to the global MCP router.
pub fn get_mcp_router() -> Arc<ToolRouter> {
    match MCP_ROUTER.get() {
        Some(router) => router.load_full(),
        None => Arc::new(ToolRouter::disabled()),
    }
}

/// Reload the MCP router with updated configuration.
pub async fn reload_mcp_router() {
    // Shutdown existing router
    if let Some(router) = MCP_ROUTER.get() {
        router.load().shutdown().await;
    }

    let config = CLEWDR_CONFIG.load();

    let new_router = if config.mcp_enabled && !config.mcp_servers.is_empty() {
        info!("Reloading MCP with {} configured servers", config.mcp_servers.len());

        let mut builder = ToolRouterBuilder::new().enabled(true);
        for server_config in config.mcp_servers.iter() {
            builder = builder.add_server(server_config.clone());
        }
        builder.build().await
    } else {
        ToolRouter::disabled()
    };

    if let Some(router) = MCP_ROUTER.get() {
        router.store(Arc::new(new_router));
    }
}

/// Add a new MCP server dynamically.
/// Note: This currently requires a config update and reload.
#[allow(dead_code)]
pub async fn add_mcp_server(_config: McpServerConfig) -> Result<(), String> {
    // We can't modify the router directly since it's behind Arc
    // For now, this requires a full reload
    // In a more sophisticated implementation, we could use RwLock on the registry

    warn!("Dynamic server addition requires config update and reload");
    Err("Dynamic server addition not yet supported. Update config and reload.".to_string())
}

/// Check if MCP is enabled and has any tools.
pub async fn mcp_has_tools() -> bool {
    let router = get_mcp_router();
    router.is_enabled() && !router.get_mcp_tools().await.is_empty()
}

/// Get list of available MCP tools.
pub async fn list_mcp_tools() -> Vec<String> {
    let router = get_mcp_router();
    if router.is_enabled() {
        router.registry().list_tool_names().await
    } else {
        Vec::new()
    }
}

/// Get list of connected MCP servers.
pub async fn list_mcp_servers() -> Vec<String> {
    let router = get_mcp_router();
    if router.is_enabled() {
        router.registry().list_servers().await
    } else {
        Vec::new()
    }
}

/// Shutdown all MCP connections.
pub async fn shutdown_mcp() {
    if let Some(router) = MCP_ROUTER.get() {
        info!("Shutting down MCP router");
        router.load().shutdown().await;
    }
}
