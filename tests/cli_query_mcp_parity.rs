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
fn explore_returns_source_without_prior_query() {
    let dir = fixture_project();
    let output = run_codegraph(&["explore", "runApp helper", "--max-files", "2"], dir.path());
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("### Sources"), "expected sources section:\n{out}");
    assert!(out.contains("runApp"), "expected matching symbol:\n{out}");
    assert!(out.contains("helper"), "expected related symbol:\n{out}");
}
