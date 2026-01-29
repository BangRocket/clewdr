//! MCP API endpoints for managing MCP servers and tools.

use axum::{Json, response::IntoResponse};
use serde::{Deserialize, Serialize};

use crate::{
    error::ClewdrError,
    mcp::{get_mcp_router, list_mcp_servers, list_mcp_tools, mcp_has_tools, reload_mcp_router},
};

/// Response for MCP status endpoint.
#[derive(Debug, Serialize)]
pub struct McpStatusResponse {
    pub enabled: bool,
    pub servers: Vec<String>,
    pub tools: Vec<McpToolInfo>,
}

/// Information about an MCP tool.
#[derive(Debug, Serialize)]
pub struct McpToolInfo {
    pub name: String,
    pub server: String,
    pub description: Option<String>,
}

/// Get MCP status and available tools.
pub async fn get_mcp_status() -> Result<Json<McpStatusResponse>, ClewdrError> {
    let router = get_mcp_router();
    let enabled = router.is_enabled();
    let servers = list_mcp_servers().await;

    let tools: Vec<McpToolInfo> = if enabled {
        let tool_names = list_mcp_tools().await;
        let mut tool_infos = Vec::new();

        for name in tool_names {
            if let Some(tool) = router.registry().lookup_tool(&name).await {
                tool_infos.push(McpToolInfo {
                    name: tool.qualified_name,
                    server: tool.server_name,
                    description: tool.tool.description,
                });
            }
        }
        tool_infos
    } else {
        Vec::new()
    };

    Ok(Json(McpStatusResponse {
        enabled,
        servers,
        tools,
    }))
}

/// Request to call an MCP tool.
#[derive(Debug, Deserialize)]
pub struct CallToolRequest {
    pub tool: String,
    pub arguments: Option<serde_json::Value>,
}

/// Response from calling an MCP tool.
#[derive(Debug, Serialize)]
pub struct CallToolResponse {
    pub success: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// Call an MCP tool directly.
pub async fn call_mcp_tool(
    Json(request): Json<CallToolRequest>,
) -> Result<Json<CallToolResponse>, ClewdrError> {
    let router = get_mcp_router();

    if !router.is_enabled() {
        return Ok(Json(CallToolResponse {
            success: false,
            result: None,
            error: Some("MCP is not enabled".to_string()),
        }));
    }

    match router.execute_if_mcp(&request.tool, request.arguments).await {
        Some(Ok(result)) => Ok(Json(CallToolResponse {
            success: true,
            result: Some(result),
            error: None,
        })),
        Some(Err(e)) => Ok(Json(CallToolResponse {
            success: false,
            result: None,
            error: Some(e),
        })),
        None => Ok(Json(CallToolResponse {
            success: false,
            result: None,
            error: Some(format!("Tool '{}' not found in MCP registry", request.tool)),
        })),
    }
}

/// Reload MCP configuration.
pub async fn reload_mcp() -> Result<Json<McpStatusResponse>, ClewdrError> {
    reload_mcp_router().await;
    get_mcp_status().await
}

/// Check if MCP has any available tools.
pub async fn mcp_health() -> impl IntoResponse {
    let has_tools = mcp_has_tools().await;
    Json(serde_json::json!({
        "healthy": has_tools,
        "mcp_enabled": get_mcp_router().is_enabled()
    }))
}
