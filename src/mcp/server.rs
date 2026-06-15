//! MCP server request dispatcher for stdio JSON-RPC clients.

use crate::db::QueryBuilder;
use crate::mcp::protocol::*;
use crate::mcp::tools;
use crate::mcp::transport::{StdioTransport, Transport};
use log::{debug, info};
use serde_json::{json, Value};

/// MCP Server implementation
pub struct MCPServer<'a> {
    transport: Box<dyn Transport>,
    project: Option<crate::project::ProjectContext>,
    queries: Option<QueryBuilder<'a>>,
}

impl<'a> MCPServer<'a> {
    pub fn new() -> Self {
        Self {
            transport: Box::new(StdioTransport::new()),
            project: None,
            queries: None,
        }
    }

    /// Create server with custom transport
    pub fn with_transport(transport: Box<dyn Transport>) -> Self {
        Self {
            transport,
            project: None,
            queries: None,
        }
    }

    /// Initialize server with the resolved project context.
    pub fn with_project(mut self, project: crate::project::ProjectContext) -> Self {
        self.project = Some(project);
        self
    }

    /// Initialize server with database connection
    pub fn with_queries(mut self, queries: QueryBuilder<'a>) -> Self {
        self.queries = Some(queries);
        self
    }

    /// Run the MCP server main loop
    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting MCP server");

        loop {
            match self.transport.read_request()? {
                Some(request) => {
                    self.handle_request(request)?;
                }
                None => {
                    // EOF - client disconnected
                    info!("Client disconnected");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle an incoming JSON-RPC request
    fn handle_request(&self, request: JsonRpcRequest) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Handling method: {}", request.method);

        let response = match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id, request.params),
            "initialized" => {
                // Notification, no response needed
                return Ok(());
            }
            "tools/list" => self.handle_tools_list(request.id),
            "tools/call" => self.handle_tools_call(request.id, request.params),
            "ping" => self.handle_ping(request.id),
            _ => {
                let error = JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", request.method),
                    data: None,
                };
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(error),
                }
            }
        };

        self.transport.write_response(&response)?;
        Ok(())
    }

    /// Handle initialize request
    fn handle_initialize(&self, id: Option<Value>, _params: Option<Value>) -> JsonRpcResponse {
        let result = InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: false,
                }),
                resources: None,
                prompts: None,
            },
            server_info: ServerInfo {
                name: "CodeGraph".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: Some(crate::mcp::tools::server_instructions(
                self.queries.is_some(),
            )),
        };

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::to_value(result).unwrap()),
            error: None,
        }
    }

    /// Handle tools/list request
    fn handle_tools_list(&self, id: Option<Value>) -> JsonRpcResponse {
        let tools_list = if self.queries.is_some() && self.project.is_some() {
            tools::register_tools()
        } else {
            Vec::new()
        };

        let result = ListToolsResult { tools: tools_list };

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::to_value(result).unwrap()),
            error: None,
        }
    }

    /// Handle tools/call request
    fn handle_tools_call(&self, id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: "Missing params".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let call_params: CallToolParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: format!("Invalid params: {}", e),
                        data: None,
                    }),
                };
            }
        };

        let project = match &self.project {
            Some(project) => project,
            None => {
                return tool_error_response(
                    id,
                    "CodeGraph is inactive. Run `codegraph init -i` first.".to_string(),
                );
            }
        };

        let queries = match &self.queries {
            Some(q) => q,
            None => {
                return tool_error_response(
                    id,
                    "CodeGraph is inactive. Run `codegraph init -i` first.".to_string(),
                );
            }
        };

        match tools::execute_tool(&call_params.name, call_params.arguments, project, queries) {
            Ok(result) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(serde_json::to_value(result).unwrap()),
                error: None,
            },
            Err(e) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32603,
                    message: format!("Tool execution failed: {}", e),
                    data: None,
                }),
            },
        }
    }

    /// Handle ping request
    fn handle_ping(&self, id: Option<Value>) -> JsonRpcResponse {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({})),
            error: None,
        }
    }
}

fn tool_error_response(id: Option<Value>, text: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(
            serde_json::to_value(CallToolResult {
                content: vec![ContentBlock {
                    content_type: "text".to_string(),
                    text,
                }],
                is_error: Some(true),
            })
            .unwrap(),
        ),
        error: None,
    }
}

impl<'a> Default for MCPServer<'a> {
    fn default() -> Self {
        Self::new()
    }
}
