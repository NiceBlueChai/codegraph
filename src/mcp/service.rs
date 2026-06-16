//! rmcp service adapter for CodeGraph MCP tools.
//!
//! The adapter keeps MCP protocol handling in the official SDK while reusing
//! the existing CodeGraph tool registry and handlers.

use std::future::Future;
use std::sync::Arc;

use crate::mcp::tools;
use crate::project::ProjectContext;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{MaybeSendFuture, RequestContext, RoleServer};
use rmcp::ErrorData as McpError;

/// MCP service implementation backed by an optional initialized CodeGraph project.
#[derive(Debug, Clone)]
pub struct CodeGraphMcpService {
    project: Option<ProjectContext>,
}

impl CodeGraphMcpService {
    /// Creates an active service for an initialized project.
    pub fn active(project: ProjectContext) -> Self {
        Self {
            project: Some(project),
        }
    }

    /// Creates an inactive service that only responds to initialization.
    pub fn inactive() -> Self {
        Self { project: None }
    }

    /// Reports whether this service has an initialized project.
    pub fn is_active(&self) -> bool {
        self.project.is_some()
    }
}

impl ServerHandler for CodeGraphMcpService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("CodeGraph", env!("CARGO_PKG_VERSION")))
            .with_instructions(tools::server_instructions(self.is_active()))
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + MaybeSendFuture + '_ {
        std::future::ready(Ok(ListToolsResult::with_all_items(if self.is_active() {
            tools::register_tools()
                .into_iter()
                .map(to_rmcp_tool)
                .collect()
        } else {
            Vec::new()
        })))
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, McpError>> + MaybeSendFuture + '_ {
        let result = match &self.project {
            Some(project) => call_project_tool(project, request),
            None => Ok(inactive_result()),
        };
        std::future::ready(result.map_err(|error| {
            McpError::internal_error(
                "Tool execution failed",
                Some(serde_json::json!({
                    "error": error.to_string(),
                })),
            )
        }))
    }
}

fn call_project_tool(
    project: &ProjectContext,
    request: CallToolRequestParams,
) -> anyhow::Result<CallToolResult> {
    let db = project.open_database()?;
    let queries = crate::db::QueryBuilder::new(db.get_conn());
    let result = tools::execute_tool(
        &request.name,
        request.arguments.map(serde_json::Value::Object),
        project,
        &queries,
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;

    Ok(to_rmcp_result(result))
}

fn inactive_result() -> CallToolResult {
    CallToolResult::error(vec![Content::text(
        "CodeGraph is inactive. Run `codegraph init -i` first.",
    )])
}

fn to_rmcp_result(result: crate::mcp::protocol::CallToolResult) -> CallToolResult {
    let content = result
        .content
        .into_iter()
        .map(|block| Content::text(block.text))
        .collect::<Vec<_>>();

    if result.is_error == Some(true) {
        CallToolResult::error(content)
    } else {
        CallToolResult::success(content)
    }
}

fn to_rmcp_tool(tool: crate::mcp::protocol::ToolDefinition) -> Tool {
    Tool::new(
        tool.name,
        tool.description,
        Arc::new(rmcp::model::object(
            serde_json::to_value(tool.input_schema).unwrap_or_else(|_| serde_json::json!({})),
        )),
    )
}
