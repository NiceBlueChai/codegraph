//! MCP tool registry, advertised schemas, server instructions, and service-backed handlers.

use crate::db::QueryBuilder;
use crate::mcp::protocol::{CallToolResult, ContentBlock, ToolDefinition, ToolInputSchema};
use crate::query_service::QueryService;
use serde_json::{json, Value};

const TOOL_PREFIX: &str = "codegraph_";
const ALL_TOOLS: &[&str] = &[
    "explore", "node", "search", "callers", "callees", "impact", "files", "status",
];
const DEFAULT_TOOLS: &[&str] = &["explore", "node", "search", "callers"];

/// Register the TypeScript-compatible MCP tool surface.
pub fn register_tools() -> Vec<ToolDefinition> {
    canonical_tool_names()
        .into_iter()
        .filter_map(tool_by_short_name)
        .collect()
}

fn canonical_tool_names() -> Vec<&'static str> {
    let requested_tools = match std::env::var("CODEGRAPH_MCP_TOOLS") {
        Ok(value) => value
            .split(',')
            .map(normalize_tool_name)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>(),
        Err(_) => return DEFAULT_TOOLS.to_vec(),
    };

    if requested_tools.is_empty() {
        return DEFAULT_TOOLS.to_vec();
    }

    ALL_TOOLS
        .iter()
        .copied()
        .filter(|name| requested_tools.iter().any(|requested| requested == name))
        .collect()
}

/// Return concise usage instructions for MCP clients.
pub fn server_instructions(active: bool) -> String {
    if active {
        "CodeGraph is active. Start with codegraph_explore to understand relevant files, use \
         codegraph_node for file snippets or symbol details, codegraph_search to locate code, and \
         codegraph_callers to inspect call sites."
            .to_string()
    } else {
        "CodeGraph is inactive for this workspace. Run `codegraph init -i` in the project before \
         using MCP tools."
            .to_string()
    }
}

fn normalize_tool_name(name: &str) -> &str {
    name.trim().strip_prefix(TOOL_PREFIX).unwrap_or(name.trim())
}

fn tool_by_short_name(name: &str) -> Option<ToolDefinition> {
    match name {
        "explore" => Some(explore_tool()),
        "node" => Some(node_tool()),
        "search" => Some(search_tool()),
        "callers" => Some(callers_tool()),
        "callees" => Some(callees_tool()),
        "impact" => Some(impact_tool()),
        "files" => Some(files_tool()),
        "status" => Some(status_tool()),
        _ => None,
    }
}

/// Explore tool: Find relevant files and symbols for a natural-language query.
fn explore_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_explore".to_string(),
        description:
            "Explore relevant code files and symbols for a query before drilling into details."
                .to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({
                "query": {
                    "type": "string",
                    "description": "Question, feature, or symbol area to explore"
                },
                "maxFiles": {
                    "type": "integer",
                    "description": "Maximum number of files to include"
                },
                "projectPath": {
                    "type": "string",
                    "description": "Project path to target instead of the current working directory"
                }
            })),
            required: Some(vec!["query".to_string()]),
        },
    }
}

/// Node tool: Inspect a file range or symbol details.
fn node_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_node".to_string(),
        description: "Inspect source for a file or retrieve details for symbols in the code graph."
            .to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({
                "symbol": {
                    "type": "string",
                    "description": "Symbol name to inspect"
                },
                "file": {
                    "type": "string",
                    "description": "File path to read from the indexed project"
                },
                "line": {
                    "type": "integer",
                    "description": "One-based line number to inspect"
                },
                "offset": {
                    "type": "integer",
                    "description": "One-based starting line offset for file reads"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines or symbols to return"
                },
                "symbolsOnly": {
                    "type": "boolean",
                    "description": "Return symbol summaries without source text"
                },
                "includeCode": {
                    "type": "boolean",
                    "description": "Include source code when returning symbol details"
                },
                "projectPath": {
                    "type": "string",
                    "description": "Project path to target instead of the current working directory"
                }
            })),
            required: Some(vec![]),
        },
    }
}

/// Callers tool: Find who calls a function
fn callers_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_callers".to_string(),
        description: "Find call sites that reference a function, method, or symbol.".to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({
                "symbol": {
                    "type": "string",
                    "description": "Function, method, or symbol to find callers for"
                },
                "file": {
                    "type": "string",
                    "description": "Optional file path to disambiguate the symbol"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of callers to return"
                },
                "projectPath": {
                    "type": "string",
                    "description": "Project path to target instead of the current working directory"
                }
            })),
            required: Some(vec!["symbol".to_string()]),
        },
    }
}

/// Callees tool: Find what a function calls
fn callees_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_callees".to_string(),
        description: "Find functions, methods, or symbols called by a target symbol.".to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({
                "symbol": {
                    "type": "string",
                    "description": "Function, method, or symbol to find callees for"
                },
                "file": {
                    "type": "string",
                    "description": "Optional file path to disambiguate the symbol"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of callees to return"
                },
                "projectPath": {
                    "type": "string",
                    "description": "Project path to target instead of the current working directory"
                }
            })),
            required: Some(vec!["symbol".to_string()]),
        },
    }
}

/// Impact analysis tool
fn impact_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_impact".to_string(),
        description: "Analyze the upstream impact of changing a function, method, or symbol."
            .to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({
                "symbol": {
                    "type": "string",
                    "description": "Function, method, or symbol to analyze"
                },
                "file": {
                    "type": "string",
                    "description": "Optional file path to disambiguate the symbol"
                },
                "depth": {
                    "type": "integer",
                    "description": "Maximum relationship depth to traverse"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of impacted nodes to return"
                },
                "projectPath": {
                    "type": "string",
                    "description": "Project path to target instead of the current working directory"
                }
            })),
            required: Some(vec!["symbol".to_string()]),
        },
    }
}

/// Search tool: Full-text search
fn search_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_search".to_string(),
        description: "Search indexed code text and symbols to locate relevant definitions."
            .to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({
                "query": {
                    "type": "string",
                    "description": "Search query text or symbol fragment"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of search results to return"
                },
                "kind": {
                    "type": "string",
                    "description": "Optional symbol kind filter"
                },
                "projectPath": {
                    "type": "string",
                    "description": "Project path to target instead of the current working directory"
                }
            })),
            required: Some(vec!["query".to_string()]),
        },
    }
}

/// Files tool: List indexed files with optional filtering and formatting.
fn files_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_files".to_string(),
        description: "List indexed files with optional filters and output controls.".to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({
                "filter": {
                    "type": "string",
                    "description": "Text filter for indexed file paths"
                },
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern used to match file paths"
                },
                "format": {
                    "type": "string",
                    "description": "Output shape for file listings",
                    "enum": ["tree", "flat"]
                },
                "maxDepth": {
                    "type": "integer",
                    "description": "Maximum directory depth for tree output"
                },
                "grouped": {
                    "type": "boolean",
                    "description": "Group files by directory or language when supported"
                },
                "noMetadata": {
                    "type": "boolean",
                    "description": "Omit language, size, and graph metadata"
                },
                "projectPath": {
                    "type": "string",
                    "description": "Project path to target instead of the current working directory"
                }
            })),
            required: Some(vec![]),
        },
    }
}

/// Status tool: Report CodeGraph workspace index status.
fn status_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_status".to_string(),
        description: "Show whether CodeGraph is initialized and summarize indexed graph metadata."
            .to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({
                "projectPath": {
                    "type": "string",
                    "description": "Project path to target instead of the current working directory"
                }
            })),
            required: Some(vec![]),
        },
    }
}

/// Execute a tool call
pub fn execute_tool<'a>(
    tool_name: &str,
    arguments: Option<Value>,
    project: &crate::project::ProjectContext,
    queries: &QueryBuilder<'a>,
) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    if !is_known_tool_name(tool_name) {
        return text_error(format!("Unknown tool: {}", tool_name));
    }
    if !register_tools().iter().any(|tool| tool.name == tool_name) {
        return text_error(format!(
            "Tool {} is disabled via CODEGRAPH_MCP_TOOLS",
            tool_name
        ));
    }

    let args = arguments.unwrap_or_else(|| json!({}));
    let service = QueryService::new(project.clone(), QueryBuilder::new(queries.get_conn()));

    match tool_name {
        "codegraph_explore" => handle_explore(&service, &args),
        "codegraph_node" => handle_node(&service, &args),
        "codegraph_search" => handle_search(&service, &args),
        "codegraph_callers" => handle_callers(&service, &args),
        "codegraph_callees" => handle_callees(&service, &args),
        "codegraph_impact" => handle_impact(&service, &args),
        "codegraph_files" => handle_files(&service, &args),
        "codegraph_status" => handle_status(&service),
        _ => text_error(format!("Unknown tool: {}", tool_name)),
    }
}

fn handle_explore(
    service: &QueryService<'_>,
    args: &Value,
) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    let query = match required_string(args, "query") {
        Ok(query) => query,
        Err(error) => return text_error(error),
    };
    let max_files = args["maxFiles"].as_u64().unwrap_or(5) as usize;

    text(service.render_explore_text(query, max_files)?)
}

fn handle_node(
    service: &QueryService<'_>,
    args: &Value,
) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    let offset = args["offset"]
        .as_u64()
        .or_else(|| args["line"].as_u64())
        .map(|n| n as usize);
    let limit = args["limit"].as_u64().map(|n| n as usize);
    let symbols_only = args["symbolsOnly"].as_bool().unwrap_or(false);

    if let Some(file) = args["file"].as_str() {
        if symbols_only {
            return text(service.render_symbols_only_text(file)?);
        }
        let view = service.file_view(file, offset, limit)?;
        return text(service.render_file_view_text(&view));
    }

    if let Some(symbol) = args["symbol"].as_str() {
        let max_files = if args["includeCode"].as_bool().unwrap_or(true) {
            1
        } else {
            0
        };
        return text(service.render_explore_text(symbol, max_files)?);
    }

    text_error("Pass either `file` or `symbol`.".to_string())
}

fn handle_search(
    service: &QueryService<'_>,
    args: &Value,
) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    let query = match required_string(args, "query") {
        Ok(query) => query,
        Err(error) => return text_error(error),
    };
    let limit = args["limit"].as_u64().unwrap_or(20) as usize;
    let kind = args["kind"].as_str();
    let results = service.search(query, limit, kind)?;
    let output = json!({
        "query": query,
        "results": results.iter().map(|result| json!({
            "name": result.node.name,
            "kind": result.node.kind.as_str(),
            "file": result.node.file_path,
            "line": result.node.start_line,
            "signature": result.node.signature,
        })).collect::<Vec<_>>(),
        "total": results.len(),
    });

    text(serde_json::to_string_pretty(&output)?)
}

fn handle_callers(
    service: &QueryService<'_>,
    args: &Value,
) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    let symbol = match required_string(args, "symbol") {
        Ok(symbol) => symbol,
        Err(error) => return text_error(error),
    };
    let limit = args["limit"].as_u64().unwrap_or(20) as usize;
    let callers = service.callers(symbol, limit)?;
    let visible = callers.iter().take(limit).cloned().collect::<Vec<_>>();

    text(QueryService::render_graph_list(
        &format!("Callers of '{}'", symbol),
        &visible,
        callers.len(),
    ))
}

fn handle_callees(
    service: &QueryService<'_>,
    args: &Value,
) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    let symbol = match required_string(args, "symbol") {
        Ok(symbol) => symbol,
        Err(error) => return text_error(error),
    };
    let limit = args["limit"].as_u64().unwrap_or(20) as usize;
    let callees = service.callees(symbol, limit)?;
    let visible = callees.iter().take(limit).cloned().collect::<Vec<_>>();

    text(QueryService::render_graph_list(
        &format!("Callees of '{}'", symbol),
        &visible,
        callees.len(),
    ))
}

fn handle_impact(
    service: &QueryService<'_>,
    args: &Value,
) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    let symbol = match required_string(args, "symbol") {
        Ok(symbol) => symbol,
        Err(error) => return text_error(error),
    };
    let depth = args["depth"].as_u64().unwrap_or(3) as usize;
    let limit = args["limit"].as_u64().unwrap_or(20) as usize;
    let impact = service.impact_summary(symbol, depth)?;
    let output = json!({
        "symbol": symbol,
        "affected": impact.affected.iter().take(limit).map(|node| json!({
            "name": node.name,
            "filePath": node.file_path,
            "startLine": node.start_line,
            "kind": node.kind.as_str(),
        })).collect::<Vec<_>>(),
        "total": impact.affected.len(),
        "edgeCount": impact.edge_count,
    });

    text(serde_json::to_string_pretty(&output)?)
}

fn handle_files(
    service: &QueryService<'_>,
    args: &Value,
) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    let include_metadata = !args["noMetadata"].as_bool().unwrap_or(false);
    let listing = service.list_files(
        args["filter"].as_str(),
        args["pattern"].as_str(),
        include_metadata,
    )?;

    text(serde_json::to_string_pretty(&listing)?)
}

fn handle_status(service: &QueryService<'_>) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    let stats = service.queries.get_stats()?;
    let output = json!({
        "initialized": true,
        "root": service.project.root.display().to_string(),
        "nodes": stats.node_count,
        "edges": stats.edge_count,
        "files": stats.file_count,
        "unresolved_refs": stats.unresolved_ref_count,
    });

    text(serde_json::to_string_pretty(&output)?)
}

fn text(text: String) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    Ok(CallToolResult {
        content: vec![ContentBlock {
            content_type: "text".to_string(),
            text,
        }],
        is_error: None,
    })
}

fn text_error(text: String) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    Ok(CallToolResult {
        content: vec![ContentBlock {
            content_type: "text".to_string(),
            text,
        }],
        is_error: Some(true),
    })
}

fn required_string<'a>(args: &'a Value, name: &str) -> Result<&'a str, String> {
    args[name]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Missing required argument `{}`", name))
}

fn is_known_tool_name(name: &str) -> bool {
    name.strip_prefix(TOOL_PREFIX)
        .map(|short| ALL_TOOLS.contains(&short))
        .unwrap_or(false)
}
