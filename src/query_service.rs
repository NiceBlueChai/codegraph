//! Shared query operations for CLI and MCP command parity.
//!
//! This module owns user-visible query behavior so the CLI and MCP tools do not
//! drift apart while the Rust rewrite catches up with the TypeScript version.

use std::collections::HashSet;
use std::path::Path;

use globset::Glob;
use serde::Serialize;

use crate::context_formatter;
use crate::core::query::GraphTraverser;
use crate::db::QueryBuilder;
use crate::project::ProjectContext;
use crate::types::{Edge, FileRecord, Node, SearchOptions, SearchResult};

/// Source content and dependency metadata for an indexed file.
#[derive(Debug, Serialize)]
pub struct FileView {
    /// Indexed project-relative file path.
    pub path: String,
    /// One-based starting line used when rendering source.
    pub offset: usize,
    /// Optional maximum number of lines to render.
    pub limit: Option<usize>,
    /// Full source content read from disk.
    pub source: String,
    /// Indexed files that depend on this file.
    pub dependents: Vec<String>,
}

/// JSON-serializable indexed file listing.
#[derive(Debug, Serialize)]
pub struct FilesListing {
    /// Resolved project root.
    pub root: String,
    /// Matching indexed files.
    pub files: Vec<FileListingEntry>,
    /// Number of matching indexed files.
    pub count: usize,
}

/// JSON-serializable indexed file metadata.
#[derive(Debug, Serialize)]
pub struct FileListingEntry {
    /// Indexed project-relative file path.
    pub path: String,
    /// File language name.
    pub language: String,
    /// Number of indexed symbols in the file.
    pub node_count: u32,
    /// File size in bytes.
    pub size: u64,
}

/// Shared query facade used by CLI commands and MCP handlers.
pub struct QueryService<'a> {
    /// Resolved project context for path-sensitive operations.
    pub project: ProjectContext,
    /// Database query builder for the resolved project.
    pub queries: QueryBuilder<'a>,
}

impl<'a> QueryService<'a> {
    /// Creates a service over an existing query builder.
    pub fn new(project: ProjectContext, queries: QueryBuilder<'a>) -> Self {
        Self { project, queries }
    }

    /// Searches indexed symbols by free-text query.
    pub fn search(
        &self,
        query: &str,
        limit: usize,
        kind: Option<&str>,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let options = SearchOptions {
            limit,
            kinds: kind.map(|k| {
                k.split(',')
                    .filter_map(|s| crate::types::NodeKind::from_str(s.trim()))
                    .collect()
            }),
            file_pattern: None,
        };
        Ok(self.queries.search_nodes(query, Some(&options))?)
    }

    /// Finds exact symbol-name matches, falling back to the best search result.
    pub fn exact_symbol_matches(&self, symbol: &str, limit: usize) -> anyhow::Result<Vec<Node>> {
        let options = SearchOptions {
            limit: 50,
            kinds: None,
            file_pattern: None,
        };
        let results = self.queries.search_nodes(symbol, Some(&options))?;
        let mut matches: Vec<Node> = results
            .iter()
            .filter(|result| {
                result.node.name == symbol
                    || result
                        .node
                        .qualified_name
                        .ends_with(&format!("::{}", symbol))
                    || result.node.name.ends_with(&format!(".{}", symbol))
            })
            .map(|result| result.node.clone())
            .collect();
        if matches.is_empty() {
            matches = results
                .into_iter()
                .take(1)
                .map(|result| result.node)
                .collect();
        }
        matches.truncate(limit);
        Ok(matches)
    }

    /// Reads an indexed file by exact path, suffix, or basename hint.
    pub fn file_view(
        &self,
        file_hint: &str,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> anyhow::Result<FileView> {
        let file = self.resolve_file_hint(file_hint)?;
        let full_path = self.project.root.join(&file);
        let source = crate::util::read_file_content(&full_path)
            .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", full_path.display(), e))?;
        let dependents = self.queries.get_file_dependents(&file).unwrap_or_default();
        Ok(FileView {
            path: file,
            offset: offset.unwrap_or(1).max(1),
            limit,
            source,
            dependents,
        })
    }

    /// Lists indexed symbols contained in a file resolved from a user hint.
    pub fn symbols_in_file(&self, file_hint: &str) -> anyhow::Result<Vec<Node>> {
        let file = self.resolve_file_hint(file_hint)?;
        Ok(self.queries.get_nodes_by_file(&file)?)
    }

    /// Lists indexed files with optional prefix and glob filtering.
    pub fn list_files(
        &self,
        filter: Option<&str>,
        pattern: Option<&str>,
    ) -> anyhow::Result<FilesListing> {
        let mut files = self.queries.get_all_files()?;
        let matcher = match pattern {
            Some(pattern) => Some(Glob::new(pattern)?.compile_matcher()),
            None => None,
        };

        files.retain(|file| {
            let normalized_path = normalize_path(&file.path);
            let filter_matches = filter
                .map(|filter| normalized_path.starts_with(&normalize_path(filter)))
                .unwrap_or(true);
            let pattern_matches = matcher
                .as_ref()
                .map(|matcher| {
                    matcher.is_match(normalized_path.as_str())
                        || matcher.is_match(file_name(&file.path))
                })
                .unwrap_or(true);
            filter_matches && pattern_matches
        });
        files.sort_by(|a, b| a.path.cmp(&b.path));

        let entries = files
            .into_iter()
            .map(|file| FileListingEntry {
                path: file.path,
                language: file.language.as_str().to_string(),
                node_count: file.node_count,
                size: file.size,
            })
            .collect::<Vec<_>>();

        Ok(FilesListing {
            root: self.project.root.display().to_string(),
            count: entries.len(),
            files: entries,
        })
    }

    /// Renders an indexed file view as Markdown with tab-separated line numbers.
    pub fn render_file_view_text(&self, view: &FileView) -> String {
        let mut out = String::new();
        out.push_str(&format!("### {}\n", view.path));
        out.push_str(&context_formatter::numbered_lines(
            &view.source,
            view.offset,
            view.limit,
        ));
        if !view.dependents.is_empty() {
            out.push_str("\n### Dependents\n");
            for dependent in &view.dependents {
                out.push_str(&format!("- {}\n", dependent));
            }
        }
        out
    }

    /// Renders the symbol outline for an indexed file.
    pub fn render_symbols_only_text(&self, file_hint: &str) -> anyhow::Result<String> {
        let file = self.resolve_file_hint(file_hint)?;
        let nodes = self.queries.get_nodes_by_file(&file)?;
        let mut out = String::new();
        out.push_str(&format!("### Symbols in {}\n", file));
        for node in nodes {
            out.push_str(&format!(
                "- {} ({}) at line {}\n",
                node.name,
                node.kind.as_str(),
                node.start_line
            ));
        }
        Ok(out)
    }

    /// Renders an explore response with symbols, relationships, and source snippets.
    pub fn render_explore_text(&self, query: &str, max_files: usize) -> anyhow::Result<String> {
        let results = self.search(query, 20, None)?;
        if results.is_empty() {
            return Ok(format!("No results found for '{}'\n", query));
        }

        let traverser = GraphTraverser::new(QueryBuilder::new(self.queries.get_conn()));
        let mut out = String::new();
        out.push_str(&format!("## Explore: {}\n\n", query));
        out.push_str("### Symbols\n");
        for result in &results {
            let node = &result.node;
            out.push_str(&format!(
                "- {} ({}) at {}:{}\n",
                node.name,
                node.kind.as_str(),
                node.file_path,
                node.start_line
            ));

            let callers = traverser.get_callers(&node.id, 1).unwrap_or_default();
            if !callers.is_empty() {
                out.push_str("  Called by:\n");
                for (caller, _) in callers.iter().take(5) {
                    out.push_str(&format!(
                        "  - {} ({}) at {}:{}\n",
                        caller.name,
                        caller.kind.as_str(),
                        caller.file_path,
                        caller.start_line
                    ));
                }
            }

            let callees = traverser.get_callees(&node.id, 1).unwrap_or_default();
            if !callees.is_empty() {
                out.push_str("  Calls:\n");
                for (callee, _) in callees.iter().take(5) {
                    out.push_str(&format!(
                        "  - {} ({}) at {}:{}\n",
                        callee.name,
                        callee.kind.as_str(),
                        callee.file_path,
                        callee.start_line
                    ));
                }
            }
        }

        out.push_str("\n### Sources\n");
        let mut files_shown = 0;
        let mut seen_files = HashSet::new();
        for result in &results {
            if files_shown >= max_files {
                break;
            }
            let node = &result.node;
            if !seen_files.insert(node.file_path.clone()) {
                continue;
            }

            let view = self.file_view(
                &node.file_path,
                Some((node.start_line as usize).saturating_sub(1).max(1)),
                Some(source_window_len(node)),
            )?;
            out.push('\n');
            out.push_str(&context_formatter::symbol_heading(node));
            out.push('\n');
            out.push_str(&context_formatter::numbered_lines(
                &view.source,
                view.offset,
                view.limit,
            ));
            files_shown += 1;
        }

        Ok(out)
    }

    /// Finds callers of matching symbols without wiring CLI graph commands yet.
    pub fn callers(&self, symbol: &str, limit: usize) -> anyhow::Result<Vec<(Node, Edge)>> {
        let matches = self.exact_symbol_matches(symbol, 50)?;
        let traverser = GraphTraverser::new(QueryBuilder::new(self.queries.get_conn()));
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for node in matches {
            let callers = traverser
                .get_callers(&node.id, 1)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            for (caller, edge) in callers {
                if seen.insert(caller.id.clone()) {
                    out.push((caller, edge));
                }
            }
        }
        out.truncate(limit);
        Ok(out)
    }

    /// Resolves a user-supplied file hint to an indexed project-relative path.
    pub fn resolve_file_hint(&self, file_hint: &str) -> anyhow::Result<String> {
        let files = self.queries.get_all_files()?;
        resolve_file_hint_from_records(file_hint, &files)
    }
}

fn resolve_file_hint_from_records(file_hint: &str, files: &[FileRecord]) -> anyhow::Result<String> {
    let hint = normalize_path(file_hint);
    if hint.is_empty() {
        anyhow::bail!("File hint is empty");
    }

    let mut exact = Vec::new();
    let mut suffix = Vec::new();
    let mut basename = Vec::new();
    for file in files {
        let path = normalize_path(&file.path);
        if path == hint {
            exact.push(file.path.clone());
        } else if path.ends_with(&format!("/{}", hint)) {
            suffix.push(file.path.clone());
        } else if file_name(&path) == hint {
            basename.push(file.path.clone());
        }
    }

    let matches = if !exact.is_empty() {
        exact
    } else if !suffix.is_empty() {
        suffix
    } else {
        basename
    };

    match matches.as_slice() {
        [single] => Ok(single.clone()),
        [] => anyhow::bail!("No indexed file matches '{}'", file_hint),
        many => anyhow::bail!(
            "File hint '{}' is ambiguous: {}",
            file_hint,
            many.join(", ")
        ),
    }
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

fn file_name(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
}

fn source_window_len(node: &Node) -> usize {
    let start = node.start_line as usize;
    let end = node.end_line.max(node.start_line) as usize;
    end.saturating_sub(start) + 3
}
