use serde_json::{json, Value};
use crate::db::QueryBuilder;
use crate::core::query::GraphTraverser;
use crate::mcp::protocol::{ToolDefinition, ToolInputSchema, CallToolResult, ContentBlock};

/// Register all available MCP tools
pub fn register_tools() -> Vec<ToolDefinition> {
    vec![
        query_tool(),
        callers_tool(),
        callees_tool(),
        impact_tool(),
        search_tool(),
        stats_tool(),
    ]
}

/// Query tool: Find symbols by name
fn query_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_query".to_string(),
        description: "Search for symbols (functions, classes, variables) by name in the codebase".to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({
                "name": {
                    "type": "string",
                    "description": "Symbol name to search for"
                },
                "kind": {
                    "type": "string",
                    "description": "Optional symbol kind filter (function, class, method, etc.)",
                    "enum": ["function", "class", "method", "interface", "struct", "variable"]
                },
                "file_pattern": {
                    "type": "string",
                    "description": "Optional file pattern to filter results"
                }
            })),
            required: Some(vec!["name".to_string()]),
        },
    }
}

/// Callers tool: Find who calls a function
fn callers_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_callers".to_string(),
        description: "Find all callers of a specific function or method".to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({
                "symbol_name": {
                    "type": "string",
                    "description": "Name of the function or method"
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum depth to search for callers (default: 1)",
                    "default": 1
                }
            })),
            required: Some(vec!["symbol_name".to_string()]),
        },
    }
}

/// Callees tool: Find what a function calls
fn callees_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_callees".to_string(),
        description: "Find all functions called by a specific function or method".to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({
                "symbol_name": {
                    "type": "string",
                    "description": "Name of the function or method"
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum depth to search for callees (default: 1)",
                    "default": 1
                }
            })),
            required: Some(vec!["symbol_name".to_string()]),
        },
    }
}

/// Impact analysis tool
fn impact_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_impact".to_string(),
        description: "Analyze the impact radius of changing a specific symbol".to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({
                "symbol_name": {
                    "type": "string",
                    "description": "Name of the symbol to analyze"
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum depth for impact analysis (default: 3)",
                    "default": 3
                }
            })),
            required: Some(vec!["symbol_name".to_string()]),
        },
    }
}

/// Search tool: Full-text search
fn search_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_search".to_string(),
        description: "Perform full-text search across the codebase".to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({
                "query": {
                    "type": "string",
                    "description": "Search query text"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 20)",
                    "default": 20
                }
            })),
            required: Some(vec!["query".to_string()]),
        },
    }
}

/// Stats tool: Get graph statistics
fn stats_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_stats".to_string(),
        description: "Get statistics about the code graph (node count, edge count, etc.)".to_string(),
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
