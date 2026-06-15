//! MCP tool registry, advertised schemas, server instructions, and legacy handlers.

use serde_json::{json, Value};
use crate::db::QueryBuilder;
use crate::core::query::GraphTraverser;
use crate::mcp::protocol::{ToolDefinition, ToolInputSchema, CallToolResult, ContentBlock};

const TOOL_PREFIX: &str = "codegraph_";
const DEFAULT_TOOLS: &[&str] = &["explore", "node", "search", "callers"];

/// Register the TypeScript-compatible MCP tool surface.
pub fn register_tools() -> Vec<ToolDefinition> {
    let requested_tools = std::env::var("CODEGRAPH_MCP_TOOLS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(normalize_tool_name)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|names| !names.is_empty())
        .unwrap_or_else(|| {
            DEFAULT_TOOLS
                .iter()
                .map(|name| (*name).to_string())
                .collect()
        });

    requested_tools
        .iter()
        .filter_map(|name| tool_by_short_name(name.as_str()))
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
        description: "Explore relevant code files and symbols for a query before drilling into details.".to_string(),
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
        description: "Inspect source for a file or retrieve details for symbols in the code graph.".to_string(),
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
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of callers to return"
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
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of callees to return"
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
        description: "Analyze the upstream impact of changing a function, method, or symbol.".to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({
                "symbol": {
                    "type": "string",
                    "description": "Function, method, or symbol to analyze"
                },
                "depth": {
                    "type": "integer",
                    "description": "Maximum relationship depth to traverse"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of impacted nodes to return"
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
        description: "Search indexed code text and symbols to locate relevant definitions.".to_string(),
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
                "noMetadata": {
                    "type": "boolean",
                    "description": "Omit language, size, and graph metadata"
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
        description: "Show whether CodeGraph is initialized and summarize indexed graph metadata.".to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({})),
            required: Some(vec![]),
        },
    }
}

/// Execute a tool call
pub fn execute_tool<'a>(
    tool_name: &str,
    arguments: Option<Value>,
    queries: &QueryBuilder<'a>,
) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    match tool_name {
        "codegraph_query" => handle_query(arguments, queries),
        "codegraph_callers" => handle_callers(arguments, queries),
        "codegraph_callees" => handle_callees(arguments, queries),
        "codegraph_impact" => handle_impact(arguments, queries),
        "codegraph_search" => handle_search(arguments, queries),
        "codegraph_stats" => handle_stats(arguments, queries),
        _ => Ok(CallToolResult {
            content: vec![ContentBlock {
                content_type: "text".to_string(),
                text: format!("Unknown tool: {}", tool_name),
            }],
            is_error: Some(true),
        }),
    }
}

fn handle_query<'a>(
    args: Option<Value>,
    queries: &QueryBuilder<'a>,
) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    let args = args.unwrap_or(json!({}));
    let name = args["name"].as_str().unwrap_or("");
    let kind_filter = args["kind"].as_str().map(|s| s.to_string());

    // Simple name-based search
    let nodes = queries.search_nodes(name, None)?;

    let mut results = Vec::new();
    for node in &nodes {
        if let Some(ref kind) = kind_filter {
            if node.node.kind.as_str() != kind {
                continue;
            }
        }

        results.push(format!(
            "- {} ({}) in {} at line {}-{}",
            node.node.name,
            node.node.kind.as_str(),
            node.node.file_path,
            node.node.start_line,
            node.node.end_line
        ));
    }

    if results.is_empty() {
        Ok(CallToolResult {
            content: vec![ContentBlock {
                content_type: "text".to_string(),
                text: format!("No symbols found matching '{}'", name),
            }],
            is_error: None,
        })
    } else {
        Ok(CallToolResult {
            content: vec![ContentBlock {
                content_type: "text".to_string(),
                text: format!("Found {} symbols:\n{}", results.len(), results.join("\n")),
            }],
            is_error: None,
        })
    }
}

fn handle_callers<'a>(
    args: Option<Value>,
    queries: &QueryBuilder<'a>,
) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    let args = args.unwrap_or(json!({}));
    let symbol_name = args["symbol_name"].as_str().unwrap_or("");
    let max_depth = args["max_depth"].as_u64().unwrap_or(1) as usize;

    // Find the node first
    let nodes = queries.search_nodes(symbol_name, None)?;
    if nodes.is_empty() {
        return Ok(CallToolResult {
            content: vec![ContentBlock {
                content_type: "text".to_string(),
                text: format!("Symbol '{}' not found", symbol_name),
            }],
            is_error: None,
        });
    }

    let traverser = GraphTraverser::new(QueryBuilder::new(queries.get_conn()));
    let node_id = &nodes[0].node.id;

    let callers = traverser.get_callers(node_id, max_depth)?;

    if callers.is_empty() {
        Ok(CallToolResult {
            content: vec![ContentBlock {
                content_type: "text".to_string(),
                text: format!("No callers found for '{}'", symbol_name),
            }],
            is_error: None,
        })
    } else {
        let caller_list: Vec<String> = callers.iter().map(|(node, _)| {
            format!("- {} in {} at line {}", node.name, node.file_path, node.start_line)
        }).collect();

        Ok(CallToolResult {
            content: vec![ContentBlock {
                content_type: "text".to_string(),
                text: format!("Callers of '{}':\n{}", symbol_name, caller_list.join("\n")),
            }],
            is_error: None,
        })
    }
}

fn handle_callees<'a>(
    args: Option<Value>,
    queries: &QueryBuilder<'a>,
) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    let args = args.unwrap_or(json!({}));
    let symbol_name = args["symbol_name"].as_str().unwrap_or("");
    let max_depth = args["max_depth"].as_u64().unwrap_or(1) as usize;

    let nodes = queries.search_nodes(symbol_name, None)?;
    if nodes.is_empty() {
        return Ok(CallToolResult {
            content: vec![ContentBlock {
                content_type: "text".to_string(),
                text: format!("Symbol '{}' not found", symbol_name),
            }],
            is_error: None,
        });
    }

    let traverser = GraphTraverser::new(QueryBuilder::new(queries.get_conn()));
    let node_id = &nodes[0].node.id;

    let callees = traverser.get_callees(node_id, max_depth)?;

    if callees.is_empty() {
        Ok(CallToolResult {
            content: vec![ContentBlock {
                content_type: "text".to_string(),
                text: format!("'{}' does not call any other functions", symbol_name),
            }],
            is_error: None,
        })
    } else {
        let callee_list: Vec<String> = callees.iter().map(|(node, _)| {
            format!("- {} in {} at line {}", node.name, node.file_path, node.start_line)
        }).collect();

        Ok(CallToolResult {
            content: vec![ContentBlock {
                content_type: "text".to_string(),
                text: format!("Functions called by '{}':\n{}", symbol_name, callee_list.join("\n")),
            }],
            is_error: None,
        })
    }
}

fn handle_impact<'a>(
    args: Option<Value>,
    queries: &QueryBuilder<'a>,
) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    let args = args.unwrap_or(json!({}));
    let symbol_name = args["symbol_name"].as_str().unwrap_or("");
    let max_depth = args["max_depth"].as_u64().unwrap_or(3) as usize;

    let nodes = queries.search_nodes(symbol_name, None)?;
    if nodes.is_empty() {
        return Ok(CallToolResult {
            content: vec![ContentBlock {
                content_type: "text".to_string(),
                text: format!("Symbol '{}' not found", symbol_name),
            }],
            is_error: None,
        });
    }

    let traverser = GraphTraverser::new(QueryBuilder::new(queries.get_conn()));
    let node_id = &nodes[0].node.id;

    let impact = traverser.get_impact_radius(node_id, max_depth)?;

    Ok(CallToolResult {
        content: vec![ContentBlock {
            content_type: "text".to_string(),
            text: format!(
                "Impact analysis for '{}':\n- Affected nodes: {}\n- Affected edges: {}",
                symbol_name,
                impact.nodes.len(),
                impact.edges.len()
            ),
        }],
        is_error: None,
    })
}

fn handle_search<'a>(
    args: Option<Value>,
    queries: &QueryBuilder<'a>,
) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    let args = args.unwrap_or(json!({}));
    let query_text = args["query"].as_str().unwrap_or("");
    let limit = args["limit"].as_u64().unwrap_or(20) as usize;

    let results = queries.full_text_search(query_text, limit)?;

    if results.is_empty() {
        Ok(CallToolResult {
            content: vec![ContentBlock {
                content_type: "text".to_string(),
                text: format!("No results found for '{}'", query_text),
            }],
            is_error: None,
        })
    } else {
        let result_list: Vec<String> = results.iter().map(|r| {
            format!(
                "- {} ({}) in {} [score: {:.2}]",
                r.node.name,
                r.node.kind.as_str(),
                r.node.file_path,
                r.score
            )
        }).collect();

        Ok(CallToolResult {
            content: vec![ContentBlock {
                content_type: "text".to_string(),
                text: format!("Search results for '{}':\n{}", query_text, result_list.join("\n")),
            }],
            is_error: None,
        })
    }
}

fn handle_stats<'a>(
    _args: Option<Value>,
    queries: &QueryBuilder<'a>,
) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    let stats = queries.get_stats()?;

    Ok(CallToolResult {
        content: vec![ContentBlock {
            content_type: "text".to_string(),
            text: format!(
                "CodeGraph Statistics:\n\
                 - Nodes: {}\n\
                 - Edges: {}\n\
                 - Files: {}\n\
                 - Unresolved references: {}",
                stats.node_count,
                stats.edge_count,
                stats.file_count,
                stats.unresolved_ref_count
            ),
        }],
        is_error: None,
    })
}
