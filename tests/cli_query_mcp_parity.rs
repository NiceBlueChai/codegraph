//! CLI and MCP parity tests for the Rust query-facing command surface.
//!
//! These tests build small indexed projects and assert stable external behavior
//! before command internals are refactored.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard, OnceLock};

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

struct McpToolsEnvGuard {
    previous: Option<OsString>,
    _lock: MutexGuard<'static, ()>,
}

fn mcp_tools_env(value: Option<&str>) -> McpToolsEnvGuard {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let lock = LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("CODEGRAPH_MCP_TOOLS env lock poisoned");
    let previous = std::env::var_os("CODEGRAPH_MCP_TOOLS");

    if let Some(value) = value {
        std::env::set_var("CODEGRAPH_MCP_TOOLS", value);
    } else {
        std::env::remove_var("CODEGRAPH_MCP_TOOLS");
    }

    McpToolsEnvGuard {
        previous,
        _lock: lock,
    }
}

impl Drop for McpToolsEnvGuard {
    fn drop(&mut self) {
        if let Some(value) = &self.previous {
            std::env::set_var("CODEGRAPH_MCP_TOOLS", value);
        } else {
            std::env::remove_var("CODEGRAPH_MCP_TOOLS");
        }
    }
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

fn fixture_project_with_two_helper_callers() -> TempDir {
    let dir = fixture_project();
    let project = codegraph::project::resolve_project(Some(
        dir.path().to_str().expect("temp path is utf-8"),
    ))
    .expect("resolve project");
    let db = project.open_database().expect("open database");
    let queries = codegraph::db::QueryBuilder::new(db.get_conn());
    let options = codegraph::types::SearchOptions {
        limit: 10,
        kinds: None,
        file_pattern: None,
    };
    let helper = queries
        .search_nodes("helper", Some(&options))
        .expect("search helper")
        .into_iter()
        .find(|result| result.node.name == "helper")
        .expect("helper node")
        .node;
    let caller_one = codegraph::types::Node::new(
        "app.ts::caller_one".to_string(),
        codegraph::types::NodeKind::Function,
        "callerOne".to_string(),
        "app.ts::callerOne".to_string(),
        "app.ts".to_string(),
        codegraph::types::Language::TypeScript,
        20,
        22,
        1,
        20,
    );
    let caller_two = codegraph::types::Node::new(
        "app.ts::caller_two".to_string(),
        codegraph::types::NodeKind::Function,
        "callerTwo".to_string(),
        "app.ts::callerTwo".to_string(),
        "app.ts".to_string(),
        codegraph::types::Language::TypeScript,
        30,
        32,
        1,
        20,
    );
    for caller in [&caller_two, &caller_one] {
        queries.insert_node(caller).expect("insert caller");
        queries
            .insert_edge(&codegraph::types::Edge::new(
                caller.id.clone(),
                helper.id.clone(),
                codegraph::types::EdgeKind::Calls,
            ))
            .expect("insert caller edge");
    }
    dir
}

fn many_matching_functions_then_class_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    let project = codegraph::project::resolve_project(Some(
        dir.path().to_str().expect("temp path is utf-8"),
    ))
    .expect("resolve project");
    let db = project.open_database().expect("open database");
    let queries = codegraph::db::QueryBuilder::new(db.get_conn());
    for i in 0..80 {
        let node = codegraph::types::Node::new(
            format!("many.ts::SearchTarget#{i}"),
            codegraph::types::NodeKind::Function,
            "SearchTarget".to_string(),
            format!("many.ts::SearchTarget::{i}"),
            "many.ts".to_string(),
            codegraph::types::Language::TypeScript,
            i + 1,
            i + 1,
            1,
            20,
        );
        queries.insert_node(&node).expect("insert function node");
    }
    let node = codegraph::types::Node::new(
        "many.ts::SearchTarget#class".to_string(),
        codegraph::types::NodeKind::Class,
        "SearchTarget".to_string(),
        "many.ts::SearchTarget::class".to_string(),
        "many.ts".to_string(),
        codegraph::types::Language::TypeScript,
        100,
        102,
        1,
        20,
    );
    queries.insert_node(&node).expect("insert class node");
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
fn files_json_no_metadata_omits_metadata_fields() {
    let dir = fixture_project();
    let output = run_codegraph(
        &["files", "--format", "flat", "--json", "--no-metadata"],
        dir.path(),
    );
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("files stdout is json");
    let file = value["files"]
        .as_array()
        .expect("files array")
        .iter()
        .find(|f| f["path"] == "app.ts")
        .expect("app.ts entry");
    assert!(file.get("path").is_some(), "expected path field:\n{file}");
    assert!(
        file.get("language").is_none(),
        "unexpected language field:\n{file}"
    );
    assert!(
        file.get("node_count").is_none(),
        "unexpected node_count field:\n{file}"
    );
    assert!(file.get("size").is_none(), "unexpected size field:\n{file}");
}

#[test]
fn query_service_filters_search_results_by_kind() {
    let dir = fixture_project();
    let project = codegraph::project::resolve_project(Some(
        dir.path().to_str().expect("temp path is utf-8"),
    ))
    .expect("resolve project");
    let db = project.open_database().expect("open database");
    let service = codegraph::query_service::QueryService::new(
        project,
        codegraph::db::QueryBuilder::new(db.get_conn()),
    );

    let results = service.search("helper", 10, Some("class")).expect("search");

    assert!(
        results.is_empty(),
        "class-filtered search should not return functions: {results:?}"
    );
}

#[test]
fn query_service_graph_methods_return_shared_relationship_results() {
    let dir = fixture_project();
    let project = codegraph::project::resolve_project(Some(
        dir.path().to_str().expect("temp path is utf-8"),
    ))
    .expect("resolve project");
    let db = project.open_database().expect("open database");
    let queries = codegraph::db::QueryBuilder::new(db.get_conn());
    let caller = codegraph::types::Node::new(
        "app.ts::caller".to_string(),
        codegraph::types::NodeKind::Function,
        "caller".to_string(),
        "app.ts::caller".to_string(),
        "app.ts".to_string(),
        codegraph::types::Language::TypeScript,
        2,
        4,
        1,
        20,
    );
    let target = codegraph::types::Node::new(
        "app.ts::target".to_string(),
        codegraph::types::NodeKind::Function,
        "target".to_string(),
        "app.ts::target".to_string(),
        "app.ts".to_string(),
        codegraph::types::Language::TypeScript,
        6,
        8,
        1,
        20,
    );
    let callee = codegraph::types::Node::new(
        "app.ts::callee".to_string(),
        codegraph::types::NodeKind::Function,
        "callee".to_string(),
        "app.ts::callee".to_string(),
        "app.ts".to_string(),
        codegraph::types::Language::TypeScript,
        10,
        12,
        1,
        20,
    );
    for node in [&caller, &target, &callee] {
        queries.insert_node(node).expect("insert node");
    }
    queries
        .insert_edge(&codegraph::types::Edge::new(
            caller.id.clone(),
            target.id.clone(),
            codegraph::types::EdgeKind::Calls,
        ))
        .expect("insert caller edge");
    queries
        .insert_edge(&codegraph::types::Edge::new(
            target.id.clone(),
            callee.id.clone(),
            codegraph::types::EdgeKind::Calls,
        ))
        .expect("insert callee edge");
    let service = codegraph::query_service::QueryService::new(project, queries);

    let callers = service.callers("target", 10).expect("callers");
    let callees = service.callees("target", 10).expect("callees");
    let impact = service.impact_nodes("target", 3).expect("impact");
    let rendered =
        codegraph::query_service::QueryService::render_graph_list("Callees", &callees, callees.len());
    let caller_names = callers.iter().map(|(node, _)| &node.name).collect::<Vec<_>>();
    let callee_names = callees.iter().map(|(node, _)| &node.name).collect::<Vec<_>>();
    let impact_names = impact.iter().map(|node| &node.name).collect::<Vec<_>>();

    assert_eq!(caller_names, vec![&caller.name]);
    assert_eq!(callee_names, vec![&callee.name]);
    assert_eq!(impact_names, vec![&caller.name]);
    assert!(
        rendered.contains("via calls"),
        "expected edge kind in rendered graph list:\n{rendered}"
    );
}

#[test]
fn callers_json_limit_does_not_change_total() {
    let dir = fixture_project_with_two_helper_callers();
    let output = run_codegraph(
        &["callers", "helper", "--limit", "1", "--json"],
        dir.path(),
    );
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("callers stdout is json");
    let callers = value["callers"].as_array().expect("callers array");

    assert_eq!(callers.len(), 1, "expected limit to apply to emitted callers:\n{value}");
    assert_eq!(callers[0]["name"], "callerOne");
    assert!(
        value["total"].as_u64().unwrap_or(0) >= 2,
        "expected total to preserve full caller count:\n{value}"
    );
}

#[test]
fn callers_text_limit_does_not_change_header_total() {
    let dir = fixture_project_with_two_helper_callers();
    let output = run_codegraph(&["callers", "helper", "--limit", "1"], dir.path());
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    let caller_rows = out.lines().filter(|line| line.starts_with("- ")).count();

    assert!(
        out.contains("Callers of 'helper' (2):"),
        "expected header to show full caller count:\n{out}"
    );
    assert_eq!(caller_rows, 1, "expected limit to apply to visible rows:\n{out}");
    assert!(out.contains("callerOne"), "expected stable first caller:\n{out}");
}

#[test]
fn impact_json_includes_edge_count() {
    let dir = fixture_project_with_two_helper_callers();
    let output = run_codegraph(&["impact", "helper", "--json"], dir.path());
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("impact stdout is json");

    assert!(
        value["edgeCount"].as_u64().is_some(),
        "expected numeric edgeCount:\n{value}"
    );
}

#[test]
fn query_kind_filter_is_applied_before_limit() {
    let dir = many_matching_functions_then_class_project();
    let output = run_codegraph(
        &["query", "SearchTarget", "--kind", "class", "--limit", "1", "--json"],
        dir.path(),
    );
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("query stdout is json");
    let results = value["results"].as_array().expect("results array");

    assert_eq!(results.len(), 1, "expected one result:\n{value}");
    assert_eq!(results[0]["kind"], "class");
}

#[test]
fn query_invalid_kind_returns_empty_json_results() {
    let dir = fixture_project();
    let output = run_codegraph(
        &["query", "helper", "--kind", "typo", "--json"],
        dir.path(),
    );
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("query stdout is json");
    let results = value["results"].as_array().expect("results array");

    assert!(results.is_empty(), "invalid kind returned results:\n{value}");
    assert_eq!(value["total"], 0);
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

#[test]
fn explore_warns_and_succeeds_when_indexed_source_is_missing() {
    let dir = fixture_project();
    fs::remove_file(dir.path().join("app.ts")).expect("remove indexed file");

    let output = run_codegraph(&["explore", "runApp", "--max-files", "2"], dir.path());

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(
        out.contains("Warning") || out.contains("failed to read"),
        "expected missing-file warning:\n{out}"
    );
}

#[test]
fn mcp_default_tool_surface_matches_typescript_default() {
    let names = {
        let _guard = mcp_tools_env(None);
        let tools = codegraph::mcp::tools::register_tools();
        tools.into_iter().map(|tool| tool.name).collect::<Vec<_>>()
    };
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
    let names = {
        let _guard = mcp_tools_env(Some("explore,node,status"));
        let tools = codegraph::mcp::tools::register_tools();
        tools.into_iter().map(|tool| tool.name).collect::<Vec<_>>()
    };
    assert_eq!(
        names,
        vec![
            "codegraph_explore",
            "codegraph_node",
            "codegraph_status",
        ]
    );
}

#[test]
fn mcp_tool_allowlist_deduplicates_in_canonical_order() {
    let names = {
        let _guard = mcp_tools_env(Some("status,node,node,explore"));
        let tools = codegraph::mcp::tools::register_tools();
        tools.into_iter().map(|tool| tool.name).collect::<Vec<_>>()
    };
    assert_eq!(
        names,
        vec![
            "codegraph_explore",
            "codegraph_node",
            "codegraph_status",
        ]
    );
}
