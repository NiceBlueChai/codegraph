<!--
  Implementation plan for the first Rust CLI parity slice.
  This plan intentionally excludes installer, affected, and upgrade work.
-->

# Rust CLI Query/MCP Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Rust `query`, `status`, `files`, `node`, `explore`, `callers`, `callees`, `impact`, and MCP query tools share one behavior-compatible service layer based on the TypeScript `main` branch.

**Architecture:** Add focused Rust modules for project resolution, query services, context formatting, and MCP tool registration. Keep `src/main.rs` as a thin command adapter and make MCP call the same service methods as CLI. This is the first implementation slice from `docs/superpowers/specs/2026-06-15-rust-cli-parity-design.md`; `install`, `uninstall`, `affected`, and `upgrade` are separate follow-up plans.

**Tech Stack:** Rust 2021, clap, rusqlite, serde/serde_json, globset, tempfile, Cargo integration tests, existing CodeGraph database/indexer modules.

---

## Scope Decomposition

The approved spec covers several independent subsystems. Implement them as separate plans:

1. This plan: query/status/files/node/explore/callers/callees/impact plus MCP parity.
2. Follow-up plan: `affected` dependency tracing.
3. Follow-up plan: `install` and `uninstall` non-interactive parity.
4. Follow-up plan: parser/framework migration slices.
5. Follow-up plan: `upgrade`; explicitly not part of first-round implementation.

## File Structure

- Create `tests/cli_query_mcp_parity.rs`: integration tests for CLI commands and MCP tool registry behavior.
- Create `src/project.rs`: project-root resolution, initialized-state checks, database opening.
- Create `src/query_service.rs`: reusable status/search/call graph/file view/files tree/explore operations.
- Create `src/context_formatter.rs`: text formatting for line-numbered source, trails, grouped files, and capped explore output.
- Modify `src/lib.rs`: export the new modules.
- Modify `src/main.rs`: replace duplicated query command logic with calls to `query_service`.
- Modify `src/mcp/protocol.rs`: add initialize `instructions` support.
- Modify `src/mcp/server.rs`: carry active/inactive project state and return no tools when inactive.
- Modify `src/mcp/tools.rs`: define TypeScript-compatible tool schemas, allowlist filtering, and service-backed handlers.

## Task 1: Add CLI Parity Test Harness

**Files:**
- Create: `tests/cli_query_mcp_parity.rs`
- Modify: none

- [ ] **Step 1: Write the failing integration test harness**

Create `tests/cli_query_mcp_parity.rs` with this content:

```rust
//! CLI and MCP parity tests for the Rust query-facing command surface.
//!
//! These tests build small indexed projects and assert stable external behavior
//! before command internals are refactored.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

fn codegraph_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_codegraph"))
}

fn run_codegraph(args: &[&str], cwd: &Path) -> Output {
    Command::new(codegraph_bin())
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("codegraph command should launch")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).replace("\r\n", "\n")
}

fn fixture_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::write(
        dir.path().join("app.ts"),
        r#"
export function helper(value: string) {
    return value.trim();
}

export function runApp(input: string) {
    return helper(input);
}
"#,
    )
    .expect("write app.ts");
    fs::create_dir_all(dir.path().join("src")).expect("create src");
    fs::write(
        dir.path().join("src").join("worker.ts"),
        r#"
export function workerMain(name: string) {
    return name.toUpperCase();
}
"#,
    )
    .expect("write worker.ts");
    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

#[test]
fn status_json_is_machine_readable_from_subdirectory() {
    let dir = fixture_project();
    let src = dir.path().join("src");
    let output = run_codegraph(&["status", "--json"], &src);
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("status stdout is json");
    assert_eq!(value["initialized"], true);
    assert!(value["files"].as_u64().unwrap_or(0) >= 2);
    assert!(value["nodes"].as_u64().unwrap_or(0) >= 2);
}

#[test]
fn node_file_mode_reads_indexed_file_with_line_numbers() {
    let dir = fixture_project();
    let output = run_codegraph(
        &[
            "node",
            "app.ts",
            "--file",
            "app.ts",
            "--offset",
            "2",
            "--limit",
            "4",
        ],
        dir.path(),
    );
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("2\t"), "expected tab line numbers:\n{out}");
    assert!(out.contains("helper"), "expected source content:\n{out}");
}

#[test]
fn files_json_has_stable_shape() {
    let dir = fixture_project();
    let output = run_codegraph(&["files", "--format", "flat", "--json"], dir.path());
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("files stdout is json");
    let files = value["files"].as_array().expect("files array");
    assert!(files.iter().any(|f| f["path"] == "app.ts"));
    assert!(files.iter().any(|f| f["language"] == "typescript"));
}

#[test]
fn explore_returns_source_without_prior_query() {
    let dir = fixture_project();
    let output = run_codegraph(&["explore", "runApp helper", "--max-files", "2"], dir.path());
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("### Sources"), "expected sources section:\n{out}");
    assert!(out.contains("runApp"), "expected matching symbol:\n{out}");
    assert!(out.contains("helper"), "expected related symbol:\n{out}");
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```powershell
cargo test --test cli_query_mcp_parity
```

Expected: at least `status_json_is_machine_readable_from_subdirectory`, `node_file_mode_reads_indexed_file_with_line_numbers`, and `explore_returns_source_without_prior_query` fail because project-root resolution, Read-shaped line numbering, and explore formatting are not yet implemented.

- [ ] **Step 3: Commit the failing tests**

```powershell
git add tests/cli_query_mcp_parity.rs
git commit -m "test(rust): 覆盖 CLI 查询行为对齐"
```

## Task 2: Add Project Resolution Service

**Files:**
- Create: `src/project.rs`
- Modify: `src/lib.rs`
- Test: `tests/cli_query_mcp_parity.rs`

- [ ] **Step 1: Create the project service**

Create `src/project.rs`:

```rust
//! Project discovery and database-opening helpers for CodeGraph commands.
//!
//! CLI and MCP entry points use this module so project-root behavior stays
//! consistent across subdirectories and Windows path spellings.

use std::path::{Path, PathBuf};

use crate::db::{get_database_path, is_initialized, DatabaseConnection};

#[derive(Debug, Clone)]
pub struct ProjectContext {
    pub root: PathBuf,
    pub db_path: PathBuf,
}

impl ProjectContext {
    pub fn open_database(&self) -> anyhow::Result<DatabaseConnection> {
        DatabaseConnection::open(
            self.db_path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Database path is not valid UTF-8: {}", self.db_path.display()))?,
        )
        .map_err(|e| anyhow::anyhow!("Failed to open database {}: {}", self.db_path.display(), e))
    }

    pub fn root_str(&self) -> anyhow::Result<&str> {
        self.root
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Project path is not valid UTF-8: {}", self.root.display()))
    }
}

pub fn resolve_project(path_arg: Option<&str>) -> anyhow::Result<ProjectContext> {
    let start = path_arg.unwrap_or(".");
    let start_path = Path::new(start);
    let absolute = if start_path.is_absolute() {
        start_path.to_path_buf()
    } else {
        std::env::current_dir()?.join(start_path)
    };
    let absolute = absolute
        .canonicalize()
        .unwrap_or_else(|_| normalize_without_existing(&absolute));

    let mut current = if absolute.is_file() {
        absolute.parent().unwrap_or(&absolute).to_path_buf()
    } else {
        absolute
    };

    loop {
        if is_initialized(path_to_str(&current)?) {
            let db_path = PathBuf::from(get_database_path(path_to_str(&current)?));
            return Ok(ProjectContext { root: current, db_path });
        }
        if !current.pop() {
            break;
        }
    }

    Err(anyhow::anyhow!(
        "CodeGraph not initialized for {}. Run 'codegraph init' first.",
        start
    ))
}

pub fn is_project_initialized(path_arg: Option<&str>) -> bool {
    resolve_project(path_arg).is_ok()
}

fn path_to_str(path: &Path) -> anyhow::Result<&str> {
    path.to_str()
        .ok_or_else(|| anyhow::anyhow!("Path is not valid UTF-8: {}", path.display()))
}

fn normalize_without_existing(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        normalized.push(component.as_os_str());
    }
    normalized
}
```

- [ ] **Step 2: Export the module**

Modify `src/lib.rs` to include:

```rust
pub mod project;
```

Keep the existing module exports unchanged.

- [ ] **Step 3: Update status to use project resolution**

In `src/main.rs`, change `cmd_status` to resolve from subdirectories:

```rust
fn cmd_status(path: &str, json: bool) -> anyhow::Result<()> {
    use std::fs;

    let project = match codegraph::project::resolve_project(Some(path)) {
        Ok(project) => project,
        Err(_) if json => {
            println!("{}", serde_json::json!({"initialized": false}));
            return Ok(());
        }
        Err(_) => {
            println!("CodeGraph not initialized");
            return Ok(());
        }
    };

    let db = project.open_database()?;
    let queries = QueryBuilder::new(db.get_conn());
    let stats = queries.get_stats()?;
    let nodes_by_kind = queries.get_nodes_by_kind_counts().unwrap_or_default();
    let languages: Vec<String> = queries.get_languages().unwrap_or_default();
    let files_by_lang = queries.get_file_counts_by_language().unwrap_or_default();
    let db_size_bytes = fs::metadata(&project.db_path).map(|m| m.len()).unwrap_or(0);

    if json {
        let output = serde_json::json!({
            "initialized": true,
            "version": env!("CARGO_PKG_VERSION"),
            "backend": format!("{:?}", db.get_backend()),
            "journal_mode": db.get_journal_mode().unwrap_or_else(|_| "unknown".to_string()),
            "nodes": stats.node_count,
            "edges": stats.edge_count,
            "files": stats.file_count,
            "unresolved_refs": stats.unresolved_ref_count,
            "db_size_bytes": db_size_bytes,
            "nodes_by_kind": nodes_by_kind.iter().map(|(k, c)| serde_json::json!({"kind": k, "count": c})).collect::<Vec<_>>(),
            "languages": languages,
            "files_by_language": files_by_lang.iter().map(|(l, c)| serde_json::json!({"language": l, "count": c})).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("CodeGraph Status:");
        println!("  Root: {}", project.root.display());
        println!("  Version: {}", env!("CARGO_PKG_VERSION"));
        println!("  Backend: {:?}", db.get_backend());
        println!("  Journal Mode: {}", db.get_journal_mode().unwrap_or_else(|_| "unknown".to_string()));
        println!("  Nodes: {}", stats.node_count);
        println!("  Edges: {}", stats.edge_count);
        println!("  Files: {}", stats.file_count);
        println!("  Unresolved Refs: {}", stats.unresolved_ref_count);
        println!("  DB Size: {:.1} MB", db_size_bytes as f64 / 1_048_576.0);
    }

    Ok(())
}
```

- [ ] **Step 4: Run the targeted status test**

Run:

```powershell
cargo test --test cli_query_mcp_parity status_json_is_machine_readable_from_subdirectory
```

Expected: PASS.

- [ ] **Step 5: Commit project resolution**

```powershell
git add src/project.rs src/lib.rs src/main.rs tests/cli_query_mcp_parity.rs
git commit -m "feat(rust): 统一查询命令项目解析"
```

## Task 3: Add Shared Query Service for Files and Symbols

**Files:**
- Create: `src/query_service.rs`
- Create: `src/context_formatter.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs`
- Test: `tests/cli_query_mcp_parity.rs`

- [ ] **Step 1: Create the formatter module**

Create `src/context_formatter.rs`:

```rust
//! Text formatters shared by CLI and MCP query responses.
//!
//! These helpers keep source snippets and file lists stable across entry points.

use crate::types::{FileRecord, Node};

pub fn numbered_lines(source: &str, offset: usize, limit: Option<usize>) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let start = offset.max(1).saturating_sub(1);
    let end = limit.map(|n| start + n).unwrap_or(lines.len()).min(lines.len());
    let mut out = String::new();
    for i in start..end {
        out.push_str(&format!("{}\t{}\n", i + 1, lines[i]));
    }
    out
}

pub fn symbol_heading(node: &Node) -> String {
    format!(
        "### {} ({})\n`{}:{}`",
        node.name,
        node.kind.as_str(),
        node.file_path,
        node.start_line
    )
}

pub fn file_entry(file: &FileRecord, include_metadata: bool) -> String {
    if include_metadata {
        format!("{} ({}, {} symbols)", file.path, file.language.as_str(), file.node_count)
    } else {
        file.path.clone()
    }
}
```

- [ ] **Step 2: Create the query service module**

Create `src/query_service.rs`:

```rust
//! Shared query operations for CLI and MCP command parity.
//!
//! This module owns user-visible query behavior so the CLI and MCP tools do not
//! drift apart while the Rust rewrite catches up with the TypeScript version.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use globset::Glob;
use serde::Serialize;

use crate::context_formatter;
use crate::core::query::GraphTraverser;
use crate::db::QueryBuilder;
use crate::project::ProjectContext;
use crate::types::{FileRecord, Node, SearchOptions, SearchResult};

#[derive(Debug, Serialize)]
pub struct FileView {
    pub path: String,
    pub offset: usize,
    pub limit: Option<usize>,
    pub source: String,
    pub dependents: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct FilesListing {
    pub root: String,
    pub files: Vec<FileListingEntry>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct FileListingEntry {
    pub path: String,
    pub language: String,
    pub node_count: u32,
    pub size: u64,
}

pub struct QueryService<'a> {
    pub project: ProjectContext,
    pub queries: QueryBuilder<'a>,
}

impl<'a> QueryService<'a> {
    pub fn new(project: ProjectContext, queries: QueryBuilder<'a>) -> Self {
        Self { project, queries }
    }

    pub fn search(&self, query: &str, limit: usize, kind: Option<&str>) -> anyhow::Result<Vec<SearchResult>> {
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

    pub fn exact_symbol_matches(&self, symbol: &str, limit: usize) -> anyhow::Result<Vec<Node>> {
        let options = SearchOptions { limit: 50, kinds: None, file_pattern: None };
        let results = self.queries.search_nodes(symbol, Some(&options))?;
        let mut matches: Vec<Node> = results
            .iter()
            .filter(|r| {
                r.node.name == symbol
                    || r.node.qualified_name.ends_with(&format!("::{}", symbol))
                    || r.node.name.ends_with(&format!(".{}", symbol))
            })
            .map(|r| r.node.clone())
            .collect();
        if matches.is_empty() {
            matches = results.into_iter().take(1).map(|r| r.node).collect();
        }
        matches.truncate(limit);
        Ok(matches)
    }

    pub fn file_view(&self, file_hint: &str, offset: Option<usize>, limit: Option<usize>) -> anyhow::Result<FileView> {
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

    pub fn symbols_in_file(&self, file_hint: &str) -> anyhow::Result<Vec<Node>> {
        let file = self.resolve_file_hint(file_hint)?;
        Ok(self.queries.get_nodes_by_file(&file)?)
    }

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
            filter.map(|f| file.path.starts_with(f)).unwrap_or(true)
                && matcher
                    .as_ref()
                    .map(|m| m.is_match(file.path.as_str()))
                    .unwrap_or(true)
        });
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let entries = files
            .into_iter()
            .map(|f| FileListingEntry {
                path: f.path,
                language: f.language.as_str().to_string(),
                node_count: f.node_count,
                size: f.size,
            })
            .collect::<Vec<_>>();
        Ok(FilesListing {
            root: self.project.root.display().to_string(),
            count: entries.len(),
            files: entries,
        })
    }

    pub fn render_file_view_text(&self, view: &FileView) -> String {
        let mut out = String::new();
        out.push_str(&format!("### {}\n", view.path));
        out.push_str(&context_formatter::numbered_lines(&view.source, view.offset, view.limit));
        if !view.dependents.is_empty() {
            out.push_str("\n### Dependents\n");
            for dependent in &view.dependents {
                out.push_str(&format!("- {}\n", dependent));
            }
        }
        out
    }

    pub fn render_symbols_only_text(&self, file_hint: &str) -> anyhow::Result<String> {
        let nodes = self.symbols_in_file(file_hint)?;
        let mut out = String::new();
        out.push_str(&format!("### Symbols in {}\n", file_hint));
        for node in nodes {
            out.push_str(&format!("- {} ({}) at line {}\n", node.name, node.kind.as_str(), node.start_line));
        }
        Ok(out)
    }

    pub fn render_explore_text(&self, query: &str, max_files: usize) -> anyhow::Result<String> {
        let results = self.search(query, 20, None)?;
        if results.is_empty() {
            return Ok(format!("No results found for '{}'", query));
        }
        let mut by_file: BTreeMap<String, Vec<Node>> = BTreeMap::new();
        for result in results {
            by_file.entry(result.node.file_path.clone()).or_default().push(result.node);
        }
        let mut out = String::new();
        out.push_str(&format!("## Explore: {}\n\n", query));
        out.push_str("### Sources\n");
        for (shown, (file, nodes)) in by_file.into_iter().enumerate() {
            if shown >= max_files {
                out.push_str("\n_Output truncated; run codegraph_explore or codegraph_node with a specific name for more._\n");
                break;
            }
            out.push_str(&format!("\n#### {}\n", file));
            let full_path = self.project.root.join(&file);
            let source = crate::util::read_file_content(&full_path).unwrap_or_default();
            for node in nodes {
                out.push_str(&format!("\n{}\n", context_formatter::symbol_heading(&node)));
                out.push_str(&context_formatter::numbered_lines(
                    &source,
                    node.start_line as usize,
                    Some((node.end_line.saturating_sub(node.start_line) + 1) as usize),
                ));
            }
        }
        Ok(out)
    }

    pub fn callers(&self, symbol: &str, limit: usize) -> anyhow::Result<Vec<(Node, crate::types::Edge)>> {
        let matches = self.exact_symbol_matches(symbol, 50)?;
        let traverser = GraphTraverser::new(QueryBuilder::new(self.queries.get_conn()));
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for node in matches {
            for (caller, edge) in traverser.get_callers(&node.id, 1)? {
                if seen.insert(caller.id.clone()) {
                    out.push((caller, edge));
                }
            }
        }
        out.truncate(limit);
        Ok(out)
    }

    fn resolve_file_hint(&self, hint: &str) -> anyhow::Result<String> {
        let normalized = hint.replace('\\', "/");
        let files = self.queries.get_all_files()?;
        let mut matches: Vec<FileRecord> = files
            .into_iter()
            .filter(|f| {
                f.path == normalized
                    || f.path.ends_with(&format!("/{}", normalized))
                    || Path::new(&f.path)
                        .file_name()
                        .map(|name| name == normalized.as_str())
                        .unwrap_or(false)
            })
            .collect();
        matches.sort_by(|a, b| a.path.cmp(&b.path));
        match matches.len() {
            1 => Ok(matches.remove(0).path),
            0 => Err(anyhow::anyhow!("No indexed file matches '{}'", hint)),
            _ => Err(anyhow::anyhow!(
                "File '{}' is ambiguous: {}",
                hint,
                matches.iter().map(|f| f.path.as_str()).collect::<Vec<_>>().join(", ")
            )),
        }
    }
}
```

- [ ] **Step 3: Export the modules**

Modify `src/lib.rs`:

```rust
pub mod context_formatter;
pub mod query_service;
```

- [ ] **Step 4: Wire `node --file` through the service**

Replace the file-mode branch in `cmd_node` with:

```rust
    if let Some(file_path) = file {
        let project = codegraph::project::resolve_project(Some(path))?;
        let db = project.open_database()?;
        let service = codegraph::query_service::QueryService::new(project, QueryBuilder::new(db.get_conn()));
        if symbols_only {
            print!("{}", service.render_symbols_only_text(file_path)?);
        } else {
            let view = service.file_view(file_path, offset, limit)?;
            print!("{}", service.render_file_view_text(&view));
        }
        return Ok(());
    }
```

- [ ] **Step 5: Wire `files --json` through the service**

At the start of `cmd_files`, after initialization checks, replace the existing JSON branch data construction with:

```rust
    let project = codegraph::project::resolve_project(Some(path))?;
    let db = project.open_database()?;
    let service = codegraph::query_service::QueryService::new(project, QueryBuilder::new(db.get_conn()));
    let listing = service.list_files(filter, pattern)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&listing)?);
        return Ok(());
    }
```

Keep the existing text `flat`, `grouped`, and `tree` output temporarily for non-JSON paths.

- [ ] **Step 6: Wire `explore` through the service**

Replace `cmd_explore` body with:

```rust
fn cmd_explore(path: &str, query: &[String], max_files: usize) -> anyhow::Result<()> {
    let project = codegraph::project::resolve_project(Some(path))?;
    let db = project.open_database()?;
    let service = codegraph::query_service::QueryService::new(project, QueryBuilder::new(db.get_conn()));
    let search_query = query.join(" ");
    println!("{}", service.render_explore_text(&search_query, max_files)?);
    Ok(())
}
```

- [ ] **Step 7: Run the CLI tests**

Run:

```powershell
cargo test --test cli_query_mcp_parity
```

Expected: the tests in Task 1 pass.

- [ ] **Step 8: Commit shared query service**

```powershell
git add src/context_formatter.rs src/query_service.rs src/lib.rs src/main.rs tests/cli_query_mcp_parity.rs
git commit -m "feat(rust): 复用查询服务输出文件和探索结果"
```

## Task 4: Move Search and Graph Commands to Query Service

**Files:**
- Modify: `src/query_service.rs`
- Modify: `src/main.rs`
- Test: `tests/cli_query_mcp_parity.rs`

- [ ] **Step 1: Add graph command renderers**

Append these methods to `impl QueryService<'a>` in `src/query_service.rs`:

```rust
    pub fn callees(&self, symbol: &str, limit: usize) -> anyhow::Result<Vec<(Node, crate::types::Edge)>> {
        let matches = self.exact_symbol_matches(symbol, 50)?;
        let traverser = GraphTraverser::new(QueryBuilder::new(self.queries.get_conn()));
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for node in matches {
            for (callee, edge) in traverser.get_callees(&node.id, 1)? {
                if seen.insert(callee.id.clone()) {
                    out.push((callee, edge));
                }
            }
        }
        out.truncate(limit);
        Ok(out)
    }

    pub fn impact_nodes(&self, symbol: &str, depth: usize) -> anyhow::Result<Vec<Node>> {
        let depth = depth.max(1).min(10);
        let matches = self.exact_symbol_matches(symbol, 50)?;
        let match_ids: HashSet<String> = matches.iter().map(|node| node.id.clone()).collect();
        let traverser = GraphTraverser::new(QueryBuilder::new(self.queries.get_conn()));
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for node in matches {
            let impact = traverser.get_impact_radius(&node.id, depth)?;
            for (_, affected) in impact.nodes {
                if !match_ids.contains(&affected.id) && seen.insert(affected.id.clone()) {
                    out.push(affected);
                }
            }
        }
        out.sort_by(|a, b| a.file_path.cmp(&b.file_path).then(a.start_line.cmp(&b.start_line)));
        Ok(out)
    }

    pub fn render_graph_list(title: &str, items: &[(Node, crate::types::Edge)]) -> String {
        if items.is_empty() {
            return format!("{}: none\n", title);
        }
        let mut out = format!("{} ({}):\n", title, items.len());
        for (node, edge) in items {
            out.push_str(&format!(
                "- {} ({}) at {}:{} via {}\n",
                node.name,
                node.kind.as_str(),
                node.file_path,
                node.start_line,
                edge.kind.as_str()
            ));
        }
        out
    }
```

- [ ] **Step 2: Replace `cmd_query` internals**

Change `cmd_query` to:

```rust
fn cmd_query(path: &str, query: &str, limit: usize, kind: Option<&str>, json: bool) -> anyhow::Result<()> {
    let project = codegraph::project::resolve_project(Some(path))?;
    let db = project.open_database()?;
    let service = codegraph::query_service::QueryService::new(project, QueryBuilder::new(db.get_conn()));
    let results = service.search(query, limit, kind)?;

    if json {
        let output = serde_json::json!({
            "query": query,
            "results": results.iter().map(|r| {
                serde_json::json!({
                    "name": r.node.name,
                    "kind": r.node.kind.as_str(),
                    "file": r.node.file_path,
                    "line": r.node.start_line,
                    "signature": r.node.signature,
                })
            }).collect::<Vec<_>>(),
            "total": results.len(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if results.is_empty() {
        println!("No results found for '{}'", query);
    } else {
        println!("Found {} results for '{}':", results.len(), query);
        for result in results {
            println!("  - {} ({}) in {}", result.node.name, result.node.kind.as_str(), result.node.file_path);
            if let Some(sig) = result.node.signature {
                println!("    Signature: {}", sig);
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Replace callers/callees/impact internals**

Use this pattern for `cmd_callers`:

```rust
fn cmd_callers(path: &str, symbol: &str, limit: usize, json: bool) -> anyhow::Result<()> {
    let project = codegraph::project::resolve_project(Some(path))?;
    let db = project.open_database()?;
    let service = codegraph::query_service::QueryService::new(project, QueryBuilder::new(db.get_conn()));
    let callers = service.callers(symbol, limit)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "symbol": symbol,
            "callers": callers.iter().map(|(node, edge)| serde_json::json!({
                "name": node.name,
                "filePath": node.file_path,
                "startLine": node.start_line,
                "kind": node.kind.as_str(),
                "edgeKind": edge.kind.as_str(),
            })).collect::<Vec<_>>(),
            "total": callers.len(),
        }))?);
    } else {
        print!("{}", codegraph::query_service::QueryService::render_graph_list(
            &format!("Callers of '{}'", symbol),
            &callers,
        ));
    }
    Ok(())
}
```

Apply the same shape to `cmd_callees` using `service.callees(symbol, limit)` and to `cmd_impact` using `service.impact_nodes(symbol, depth)`.

- [ ] **Step 4: Run existing graph tests**

Run:

```powershell
cargo test graph --lib
cargo test --test cli_query_mcp_parity
```

Expected: PASS. If graph tests expose a compile issue with `GraphTraverser::new`, use the public constructor instead of struct literal everywhere.

- [ ] **Step 5: Commit graph command service migration**

```powershell
git add src/query_service.rs src/main.rs tests/cli_query_mcp_parity.rs
git commit -m "refactor(rust): 统一图查询命令实现"
```

## Task 5: Add MCP Tool Allowlist and Inactive Behavior Tests

**Files:**
- Modify: `tests/cli_query_mcp_parity.rs`

- [ ] **Step 1: Add direct unit-style tests for tool registration**

Append to `tests/cli_query_mcp_parity.rs`:

```rust
#[test]
fn mcp_default_tool_surface_matches_typescript_default() {
    let tools = codegraph::mcp::tools::register_tools();
    let names = tools.into_iter().map(|tool| tool.name).collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "codegraph_explore",
            "codegraph_node",
            "codegraph_search",
            "codegraph_callers",
        ]
    );
}

#[test]
fn mcp_tool_allowlist_accepts_short_names() {
    std::env::set_var("CODEGRAPH_MCP_TOOLS", "explore,node,status");
    let tools = codegraph::mcp::tools::register_tools();
    std::env::remove_var("CODEGRAPH_MCP_TOOLS");
    let names = tools.into_iter().map(|tool| tool.name).collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "codegraph_explore",
            "codegraph_node",
            "codegraph_status",
        ]
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
cargo test --test cli_query_mcp_parity mcp_
```

Expected: FAIL because current Rust MCP registers `codegraph_query`, `codegraph_callees`, `codegraph_impact`, `codegraph_search`, and `codegraph_stats`.

- [ ] **Step 3: Commit failing MCP tests**

```powershell
git add tests/cli_query_mcp_parity.rs
git commit -m "test(rust): 覆盖 MCP 工具面默认行为"
```

## Task 6: Replace MCP Tool Registry With TypeScript-Compatible Surface

**Files:**
- Modify: `src/mcp/tools.rs`
- Modify: `src/mcp/protocol.rs`
- Modify: `src/mcp/server.rs`
- Test: `tests/cli_query_mcp_parity.rs`

- [ ] **Step 1: Add initialize instructions field**

Modify `InitializeResult` in `src/mcp/protocol.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}
```

Update `handle_initialize` in `src/mcp/server.rs` to set:

```rust
instructions: Some(crate::mcp::tools::server_instructions(self.queries.is_some())),
```

- [ ] **Step 2: Replace `register_tools` with allowlist filtering**

In `src/mcp/tools.rs`, replace `register_tools` with:

```rust
const DEFAULT_MCP_TOOLS: &[&str] = &["explore", "node", "search", "callers"];

pub fn register_tools() -> Vec<ToolDefinition> {
    let tools = vec![
        explore_tool(),
        node_tool(),
        search_tool(),
        callers_tool(),
        callees_tool(),
        impact_tool(),
        files_tool(),
        status_tool(),
    ];

    let allow = std::env::var("CODEGRAPH_MCP_TOOLS").ok().and_then(|raw| {
        let names = raw
            .split(',')
            .map(short_tool_name)
            .filter(|s| !s.is_empty())
            .collect::<std::collections::HashSet<_>>();
        if names.is_empty() { None } else { Some(names) }
    });

    tools
        .into_iter()
        .filter(|tool| {
            let short = short_tool_name(&tool.name);
            allow.as_ref()
                .map(|names| names.contains(short.as_str()))
                .unwrap_or_else(|| DEFAULT_MCP_TOOLS.contains(&short.as_str()))
        })
        .collect()
}

fn short_tool_name(name: &str) -> String {
    name.trim().trim_start_matches("codegraph_").to_string()
}
```

- [ ] **Step 3: Add TypeScript-compatible tool definitions**

Add or replace tool definition functions in `src/mcp/tools.rs`:

```rust
fn explore_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_explore".to_string(),
        description: "Primary tool: explore an area and return relevant source, relationships, and blast radius.".to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({
                "query": {"type": "string", "description": "Symbol names, file names, or a natural-language code question."},
                "maxFiles": {"type": "integer", "description": "Maximum number of files to include source from."}
            })),
            required: Some(vec!["query".to_string()]),
        },
    }
}

fn node_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_node".to_string(),
        description: "Read an indexed source file with line numbers or return one symbol's source and trail.".to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({
                "symbol": {"type": "string", "description": "Symbol name to inspect."},
                "file": {"type": "string", "description": "File path or basename to read or disambiguate."},
                "offset": {"type": "integer", "description": "1-based start line for file mode."},
                "limit": {"type": "integer", "description": "Maximum lines for file mode."},
                "symbolsOnly": {"type": "boolean", "description": "Return only the symbol outline for file mode."}
            })),
            required: Some(vec![]),
        },
    }
}

fn files_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_files".to_string(),
        description: "Show indexed project file structure.".to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({
                "filter": {"type": "string"},
                "pattern": {"type": "string"},
                "format": {"type": "string", "enum": ["tree", "flat", "grouped"]},
                "maxDepth": {"type": "integer"},
                "noMetadata": {"type": "boolean"}
            })),
            required: Some(vec![]),
        },
    }
}

fn status_tool() -> ToolDefinition {
    ToolDefinition {
        name: "codegraph_status".to_string(),
        description: "Get index status.".to_string(),
        input_schema: ToolInputSchema {
            schema_type: "object".to_string(),
            properties: Some(json!({})),
            required: Some(vec![]),
        },
    }
}
```

Keep `search_tool`, `callers_tool`, `callees_tool`, and `impact_tool`, but rename schemas to accept TypeScript-style names where needed (`query`, `symbol`, `limit`, `depth`).

- [ ] **Step 4: Add server instructions helper**

Add to `src/mcp/tools.rs`:

```rust
pub fn server_instructions(active: bool) -> String {
    if !active {
        return "CodeGraph is inactive because this workspace is not initialized. Run `codegraph init -i` in the project before using CodeGraph tools.".to_string();
    }
    [
        "CodeGraph is a pre-built semantic code index.",
        "Use `codegraph_explore` first for architecture, flow, bug, or area-understanding questions.",
        "Use `codegraph_node` to read indexed source files with line numbers or inspect one symbol.",
        "Use `codegraph_search` only to locate a symbol by name.",
        "Use `codegraph_callers` for exhaustive call-site lists.",
        "Do not grep or read files first when CodeGraph can answer from the index.",
    ].join("\n")
}
```

- [ ] **Step 5: Run MCP registry tests**

Run:

```powershell
cargo test --test cli_query_mcp_parity mcp_
```

Expected: PASS.

- [ ] **Step 6: Commit MCP registry parity**

```powershell
git add src/mcp/tools.rs src/mcp/protocol.rs src/mcp/server.rs tests/cli_query_mcp_parity.rs
git commit -m "feat(rust): 对齐 MCP 默认工具面"
```

## Task 7: Back MCP Tool Calls With Query Service

**Files:**
- Modify: `src/mcp/tools.rs`
- Modify: `src/mcp/server.rs`
- Modify: `src/main.rs`
- Test: `tests/cli_query_mcp_parity.rs`

- [ ] **Step 1: Change MCP server to store project context**

Modify `MCPServer` in `src/mcp/server.rs`:

```rust
pub struct MCPServer<'a> {
    transport: Box<dyn Transport>,
    project: Option<crate::project::ProjectContext>,
    queries: Option<QueryBuilder<'a>>,
}
```

Update constructors to set `project: None`, and add:

```rust
pub fn with_project(mut self, project: crate::project::ProjectContext) -> Self {
    self.project = Some(project);
    self
}
```

- [ ] **Step 2: Pass project context from `cmd_serve`**

In `cmd_serve`, replace the project opening logic with:

```rust
        let project = match codegraph::project::resolve_project(path) {
            Ok(project) => project,
            Err(_) => {
                let mut server = codegraph::mcp::server::MCPServer::new();
                server.run().map_err(|e| anyhow::anyhow!("MCP server error: {}", e))?;
                return Ok(());
            }
        };
        let db = project.open_database()?;
        let queries = QueryBuilder::new(db.get_conn());
        let mut server = codegraph::mcp::server::MCPServer::new()
            .with_project(project)
            .with_queries(queries);
        server.run().map_err(|e| anyhow::anyhow!("MCP server error: {}", e))?;
```

- [ ] **Step 3: Return no tools when inactive**

In `handle_tools_list`, use:

```rust
let tools_list = if self.queries.is_some() {
    tools::register_tools()
} else {
    Vec::new()
};
```

- [ ] **Step 4: Change `execute_tool` signature**

In `src/mcp/tools.rs`, change:

```rust
pub fn execute_tool<'a>(
    tool_name: &str,
    arguments: Option<Value>,
    project: &crate::project::ProjectContext,
    queries: &QueryBuilder<'a>,
) -> Result<CallToolResult, Box<dyn std::error::Error>>
```

Create the service at the start:

```rust
let service = crate::query_service::QueryService::new(project.clone(), QueryBuilder::new(queries.get_conn()));
```

Update `handle_tools_call` in `src/mcp/server.rs` to pass both `project` and `queries`. If either is missing, return a `CallToolResult` error object instead of a JSON-RPC internal error:

```rust
let project = match &self.project {
    Some(project) => project,
    None => {
        return JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::to_value(CallToolResult {
                content: vec![ContentBlock {
                    content_type: "text".to_string(),
                    text: "CodeGraph is inactive. Run `codegraph init -i` first.".to_string(),
                }],
                is_error: Some(true),
            }).unwrap()),
            error: None,
        };
    }
};
```

- [ ] **Step 5: Implement service-backed MCP handlers**

In `execute_tool`, route as follows:

```rust
match tool_name {
    "codegraph_explore" => {
        let query = args["query"].as_str().unwrap_or("");
        let max_files = args["maxFiles"].as_u64().unwrap_or(5) as usize;
        text(service.render_explore_text(query, max_files)?)
    }
    "codegraph_node" => {
        let file = args["file"].as_str();
        let symbol = args["symbol"].as_str();
        let offset = args["offset"].as_u64().map(|n| n as usize);
        let limit = args["limit"].as_u64().map(|n| n as usize);
        let symbols_only = args["symbolsOnly"].as_bool().unwrap_or(false);
        if let Some(file) = file {
            if symbols_only {
                text(service.render_symbols_only_text(file)?)
            } else {
                let view = service.file_view(file, offset, limit)?;
                text(service.render_file_view_text(&view))
            }
        } else if let Some(symbol) = symbol {
            text(service.render_explore_text(symbol, 1)?)
        } else {
            text("Pass either `file` or `symbol`.".to_string())
        }
    }
    "codegraph_search" => handle_search_with_service(&service, args),
    "codegraph_callers" => handle_callers_with_service(&service, args),
    "codegraph_callees" => handle_callees_with_service(&service, args),
    "codegraph_impact" => handle_impact_with_service(&service, args),
    "codegraph_files" => {
        let listing = service.list_files(args["filter"].as_str(), args["pattern"].as_str())?;
        text(serde_json::to_string_pretty(&listing)?)
    }
    "codegraph_status" => handle_status_with_service(&service),
    _ => text_error(format!("Unknown tool: {}", tool_name)),
}
```

Add helpers:

```rust
fn text(text: String) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    Ok(CallToolResult {
        content: vec![ContentBlock { content_type: "text".to_string(), text }],
        is_error: None,
    })
}

fn text_error(text: String) -> Result<CallToolResult, Box<dyn std::error::Error>> {
    Ok(CallToolResult {
        content: vec![ContentBlock { content_type: "text".to_string(), text }],
        is_error: Some(true),
    })
}
```

- [ ] **Step 6: Run MCP and CLI tests**

Run:

```powershell
cargo test --test cli_query_mcp_parity
cargo test --lib mcp
```

Expected: PASS.

- [ ] **Step 7: Commit MCP service-backed handlers**

```powershell
git add src/mcp/tools.rs src/mcp/server.rs src/main.rs tests/cli_query_mcp_parity.rs
git commit -m "feat(rust): 复用查询服务实现 MCP 工具"
```

## Task 8: Final Verification and Cleanup

**Files:**
- Modify only files touched by earlier tasks if verification exposes issues.

- [ ] **Step 1: Run formatting**

Run:

```powershell
cargo fmt
```

Expected: no output or normal formatting updates.

- [ ] **Step 2: Run focused tests**

Run:

```powershell
cargo test --test cli_query_mcp_parity
cargo test --lib
```

Expected: PASS.

- [ ] **Step 3: Run full Rust tests**

Run:

```powershell
cargo test
```

Expected: PASS. If existing unrelated tests fail, capture the exact failing test names and error output before deciding whether they are in this slice.

- [ ] **Step 4: Confirm `upgrade` remains out of scope**

Run:

```powershell
cargo run -- upgrade --check
```

Expected: it must not pretend to perform a real upgrade. Acceptable outputs are either a clear “not implemented in Rust migration yet” message or absence from help if a prior task removed it from clap.

- [ ] **Step 5: Review dirty files**

Run:

```powershell
git status --short
git diff --stat
```

Expected: only files from this plan are modified. Do not revert pre-existing unrelated dirty files such as local test databases.

- [ ] **Step 6: Commit final cleanup if needed**

If formatting or small cleanup changed files after the last task:

```powershell
git add src tests
git commit -m "chore(rust): 整理查询功能对齐实现"
```

