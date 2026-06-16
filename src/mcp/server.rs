//! MCP server entry point backed by the official rmcp stdio transport.

use crate::mcp::service::CodeGraphMcpService;
use crate::project::ProjectContext;

/// MCP server wrapper kept for the CLI's existing construction flow.
pub struct MCPServer {
    project: Option<ProjectContext>,
}

impl MCPServer {
    /// Creates an inactive MCP server.
    pub fn new() -> Self {
        Self { project: None }
    }

    /// Initializes the server with the resolved project context.
    pub fn with_project(mut self, project: ProjectContext) -> Self {
        self.project = Some(project);
        self
    }

    /// Compatibility shim for callers that used to inject a query builder.
    pub fn with_queries(self, _queries: crate::db::QueryBuilder<'_>) -> Self {
        self
    }

    /// Runs the MCP server on stdio.
    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let service = match self.project.clone() {
            Some(project) => CodeGraphMcpService::active(project),
            None => CodeGraphMcpService::inactive(),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        runtime.block_on(async move {
            let running = rmcp::serve_server(service, rmcp::transport::stdio()).await?;
            let _ = running.waiting().await?;
            Ok::<(), Box<dyn std::error::Error>>(())
        })
    }
}

impl Default for MCPServer {
    fn default() -> Self {
        Self::new()
    }
}
