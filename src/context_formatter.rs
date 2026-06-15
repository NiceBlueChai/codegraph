//! Text formatters shared by CLI and MCP query responses.
//!
//! These helpers keep source snippets and file lists stable across entry points.

use crate::types::{FileRecord, Node};

/// Renders source with 1-based tab-separated line numbers.
pub fn numbered_lines(source: &str, offset: usize, limit: Option<usize>) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let start = offset.max(1).saturating_sub(1);
    let end = limit
        .map(|n| start + n)
        .unwrap_or(lines.len())
        .min(lines.len());
    let mut out = String::new();
    for i in start..end {
        out.push_str(&format!("{}\t{}\n", i + 1, lines[i]));
    }
    out
}

/// Renders a stable Markdown heading for a symbol.
pub fn symbol_heading(node: &Node) -> String {
    format!(
        "### {} ({})\n`{}:{}`",
        node.name,
        node.kind.as_str(),
        node.file_path,
        node.start_line
    )
}

/// Renders one file listing entry, optionally including language and symbol count.
pub fn file_entry(file: &FileRecord, include_metadata: bool) -> String {
    if include_metadata {
        format!(
            "{} ({}, {} symbols)",
            file.path,
            file.language.as_str(),
            file.node_count
        )
    } else {
        file.path.clone()
    }
}
