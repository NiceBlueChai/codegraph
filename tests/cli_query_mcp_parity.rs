//! CLI and MCP parity tests for the Rust query-facing command surface.
//!
//! These tests build small indexed projects and assert stable external behavior
//! before command internals are refactored.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::io::Write;
use std::process::{Command, Output, Stdio};
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

fn run_codegraph_with_input(args: &[&str], cwd: &Path, input: &str) -> Output {
    let mut child = Command::new(codegraph_bin())
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("codegraph command should launch");
    child
        .stdin
        .as_mut()
        .expect("stdin pipe")
        .write_all(input.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait for codegraph")
}

fn run_codegraph_with_env(args: &[&str], cwd: &Path, envs: &[(&str, &Path)]) -> Output {
    let mut command = Command::new(codegraph_bin());
    command.args(args).current_dir(cwd);
    for (key, value) in envs {
        command.env(key, value);
    }
    command.output().expect("codegraph command should launch")
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
    let project =
        codegraph::project::resolve_project(Some(dir.path().to_str().expect("temp path is utf-8")))
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

fn cpp_call_graph_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::write(
        dir.path().join("main.cpp"),
        r#"
static int helper(int value) {
    return value + 1;
}

int run() {
    return helper(41);
}

class ImageView {
public:
    void clearIMG();
    void setROIParas() {
        this->clearIMG();
    }
};

void ImageView::clearIMG() {
}
"#,
    )
    .expect("write main.cpp");
    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn python_call_graph_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::write(
        dir.path().join("app.py"),
        r#"
def helper(value):
    return value.strip()

def run_app(value):
    return helper(value)
"#,
    )
    .expect("write app.py");
    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn go_call_graph_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::write(
        dir.path().join("main.go"),
        r#"
package main

func Helper(value string) string {
    return value
}

func Run(value string) string {
    return Helper(value)
}

type Runner struct{}

func (r *Runner) Clean(value string) string {
    return value
}

func (r *Runner) Serve(value string) string {
    return r.Clean(value)
}
"#,
    )
    .expect("write main.go");
    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn java_call_graph_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::write(
        dir.path().join("Service.java"),
        r#"
class Service {
    static String Helper(String value) {
        return value.trim();
    }

    static String Run(String value) {
        return Helper(value);
    }

    String Clean(String value) {
        return value;
    }

    String Serve(String value) {
        return this.Clean(value);
    }
}
"#,
    )
    .expect("write Service.java");
    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn csharp_call_graph_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::write(
        dir.path().join("Service.cs"),
        r#"
class Service
{
    static string Helper(string value)
    {
        return value.Trim();
    }

    static string Run(string value)
    {
        return Helper(value);
    }

    string Clean(string value)
    {
        return value;
    }

    string Serve(string value)
    {
        return this.Clean(value);
    }
}
"#,
    )
    .expect("write Service.cs");
    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn csharp_record_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::write(
        dir.path().join("types.cs"),
        "namespace P;\npublic record Box(int N);\n",
    )
    .expect("write types");
    fs::write(
        dir.path().join("use.cs"),
        concat!(
            "using System.Collections.Generic;\n",
            "namespace P;\n",
            "public class User {\n",
            "    public IEnumerable<Box> Boxes { get; }\n",
            "    public Box Make() => new Box(1);\n",
            "}\n",
        ),
    )
    .expect("write use");

    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn go_composite_literal_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::create_dir_all(dir.path().join("render")).expect("create render");
    fs::write(dir.path().join("go.mod"), "module example.com/proj\n").expect("write go.mod");
    fs::write(
        dir.path().join("render").join("xml.go"),
        "package render\n\ntype XML struct { Data any }\n",
    )
    .expect("write xml");
    fs::write(
        dir.path().join("app.go"),
        concat!(
            "package main\n\n",
            "import \"example.com/proj/render\"\n\n",
            "func Build() any {\n",
            "    return render.XML{}\n",
            "}\n",
        ),
    )
    .expect("write app");

    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn go_composite_registry_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::create_dir_all(dir.path().join("render")).expect("create render");
    fs::write(dir.path().join("go.mod"), "module example.com/proj\n").expect("write go.mod");
    fs::write(
        dir.path().join("render").join("xml.go"),
        "package render\n\ntype XML struct { Data any }\n",
    )
    .expect("write xml");
    fs::write(
        dir.path().join("reg.go"),
        concat!(
            "package main\n\n",
            "import \"example.com/proj/render\"\n\n",
            "type Renderer interface{}\n\n",
            "var registry = map[string]Renderer{\n",
            "    \"xml\": render.XML{},\n",
            "}\n",
        ),
    )
    .expect("write registry");

    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn go_pointer_conversion_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::write(dir.path().join("go.mod"), "module example.com/proj\n").expect("write go.mod");
    fs::write(
        dir.path().join("types.go"),
        "package main\n\ntype Wrapped struct { N int }\n",
    )
    .expect("write types");
    fs::write(
        dir.path().join("use.go"),
        "package main\n\nfunc run(x *int) { _ = (*Wrapped)(x) }\n",
    )
    .expect("write use");

    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn go_top_level_closure_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::write(dir.path().join("go.mod"), "module example.com/proj\n").expect("write go.mod");
    fs::write(
        dir.path().join("factory.go"),
        "package main\n\nfunc Wire() error { return nil }\n",
    )
    .expect("write factory");
    fs::write(
        dir.path().join("root.go"),
        concat!(
            "package main\n\n",
            "type Cmd struct{ RunE func() error }\n\n",
            "var rootCmd = &Cmd{\n",
            "    RunE: func() error { return Wire() },\n",
            "}\n",
        ),
    )
    .expect("write root");

    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn go_implicit_interface_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::create_dir_all(dir.path().join("codec")).expect("create codec");
    fs::write(dir.path().join("go.mod"), "module example.com/proj\n").expect("write go.mod");
    fs::write(
        dir.path().join("codec").join("api.go"),
        concat!(
            "package codec\n\n",
            "type Core interface {\n",
            "    Marshal(v any) ([]byte, error)\n",
            "}\n\n",
            "var API Core\n",
        ),
    )
    .expect("write api");
    fs::write(
        dir.path().join("codec").join("json.go"),
        concat!(
            "package codec\n\n",
            "type jsonApi struct{}\n\n",
            "func (j jsonApi) Marshal(v any) ([]byte, error) { return nil, nil }\n\n",
            "func init() { API = jsonApi{} }\n",
        ),
    )
    .expect("write json");

    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn single_file_call_graph_project(file_name: &str, source: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::write(dir.path().join(file_name), source).expect("write source file");
    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
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
    let project =
        codegraph::project::resolve_project(Some(dir.path().to_str().expect("temp path is utf-8")))
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

fn affected_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::create_dir_all(dir.path().join("src")).expect("create src");
    fs::create_dir_all(dir.path().join("tests")).expect("create tests");
    fs::write(dir.path().join("src").join("lib.ts"), "export const lib = 1;\n")
        .expect("write lib");
    fs::write(
        dir.path().join("src").join("app.ts"),
        "import { lib } from './lib';\nexport const app = lib;\n",
    )
    .expect("write app");
    fs::write(
        dir.path().join("tests").join("lib.test.ts"),
        "import { app } from '../src/app';\n",
    )
    .expect("write test");

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

    let lib = codegraph::types::Node::new(
        "src/lib.ts::lib".to_string(),
        codegraph::types::NodeKind::Variable,
        "lib".to_string(),
        "src/lib.ts::lib".to_string(),
        "src/lib.ts".to_string(),
        codegraph::types::Language::TypeScript,
        1,
        1,
        1,
        20,
    );
    let app = codegraph::types::Node::new(
        "src/app.ts::app".to_string(),
        codegraph::types::NodeKind::Variable,
        "app".to_string(),
        "src/app.ts::app".to_string(),
        "src/app.ts".to_string(),
        codegraph::types::Language::TypeScript,
        2,
        2,
        1,
        20,
    );
    let test = codegraph::types::Node::new(
        "tests/lib.test.ts::test".to_string(),
        codegraph::types::NodeKind::Function,
        "test".to_string(),
        "tests/lib.test.ts::test".to_string(),
        "tests/lib.test.ts".to_string(),
        codegraph::types::Language::TypeScript,
        1,
        1,
        1,
        20,
    );
    for node in [&lib, &app, &test] {
        queries.insert_node(node).expect("insert affected node");
    }
    queries
        .insert_edge(&codegraph::types::Edge::new(
            app.id.clone(),
            lib.id.clone(),
            codegraph::types::EdgeKind::Imports,
        ))
        .expect("insert app dependency");
    queries
        .insert_edge(&codegraph::types::Edge::new(
            test.id.clone(),
            app.id.clone(),
            codegraph::types::EdgeKind::Imports,
        ))
        .expect("insert test dependency");

    dir
}

fn ts_import_affected_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::create_dir_all(dir.path().join("src")).expect("create src");
    fs::create_dir_all(dir.path().join("tests")).expect("create tests");
    fs::write(
        dir.path().join("src").join("foo.ts"),
        "export const widget = { n: 1 };\n",
    )
    .expect("write foo");
    fs::write(
        dir.path().join("tests").join("foo.test.ts"),
        "import { widget } from '../src/foo';\nexport const registry = [widget];\n",
    )
    .expect("write test");

    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn ts_reexport_affected_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::create_dir_all(dir.path().join("src")).expect("create src");
    fs::create_dir_all(dir.path().join("tests")).expect("create tests");
    fs::write(
        dir.path().join("src").join("foo.ts"),
        "export function helper(): void {}\n",
    )
    .expect("write foo");
    fs::write(
        dir.path().join("src").join("bar.ts"),
        "export { helper } from './foo';\n",
    )
    .expect("write bar");
    fs::write(
        dir.path().join("tests").join("foo.test.ts"),
        "import { helper } from '../src/bar';\nexport const registry = [helper];\n",
    )
    .expect("write test");

    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn python_import_affected_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::create_dir_all(dir.path().join("pkg")).expect("create pkg");
    fs::write(dir.path().join("pkg").join("__init__.py"), "").expect("write init");
    fs::write(
        dir.path().join("pkg").join("foo.py"),
        "def helper():\n    return 1\n",
    )
    .expect("write foo");
    fs::write(
        dir.path().join("pkg").join("foo.test.py"),
        "from foo import helper\nregistry = [helper]\n",
    )
    .expect("write test");

    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn python_relative_module_affected_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::create_dir_all(dir.path().join("pkg")).expect("create pkg");
    fs::write(dir.path().join("pkg").join("__init__.py"), "").expect("write init");
    fs::write(
        dir.path().join("pkg").join("certs.py"),
        "def where():\n    return '/ca.pem'\n",
    )
    .expect("write certs");
    fs::write(
        dir.path().join("pkg").join("utils.test.py"),
        "from . import certs\nCA = certs.where()\n",
    )
    .expect("write test");

    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn rust_use_reexport_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::create_dir_all(dir.path().join("src").join("api")).expect("create api");
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"proj\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write cargo");
    fs::write(dir.path().join("src").join("lib.rs"), "pub mod api;\n").expect("write lib");
    fs::write(
        dir.path().join("src").join("api").join("mod.rs"),
        "mod widget;\npub use self::widget::Widget;\n",
    )
    .expect("write mod");
    fs::write(
        dir.path().join("src").join("api").join("widget.rs"),
        "pub struct Widget { pub n: i32 }\n",
    )
    .expect("write widget");

    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn rust_use_collision_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::create_dir_all(dir.path().join("src")).expect("create src");
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"proj\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write cargo");
    fs::write(
        dir.path().join("src").join("lib.rs"),
        "pub mod fast;\npub mod slow;\npub mod hub;\n",
    )
    .expect("write lib");
    fs::write(dir.path().join("src").join("fast.rs"), "pub fn read() -> i32 { 1 }\n")
        .expect("write fast");
    fs::write(dir.path().join("src").join("slow.rs"), "pub fn read() -> i32 { 2 }\n")
        .expect("write slow");
    fs::write(
        dir.path().join("src").join("hub.rs"),
        "pub use crate::fast::read;\n",
    )
    .expect("write hub");

    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn java_annotation_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::create_dir_all(dir.path().join("p")).expect("create p");
    fs::write(
        dir.path().join("p").join("MyAnno.java"),
        "package p;\npublic @interface MyAnno { String value() default \"\"; }\n",
    )
    .expect("write annotation");
    fs::write(
        dir.path().join("p").join("User.java"),
        concat!(
            "package p;\n",
            "@MyAnno(\"c\")\n",
            "public class User {\n",
            "  @MyAnno(\"f\") int field;\n",
            "  @MyAnno(\"m\") void go() {}\n",
            "}\n",
        ),
    )
    .expect("write user");

    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn swift_property_wrapper_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    let module_dir = dir.path().join("Sources").join("M");
    fs::create_dir_all(&module_dir).expect("create module dir");
    fs::write(
        module_dir.join("Wrap.swift"),
        "@propertyWrapper\npublic struct Argument<T> { public var wrappedValue: T }\n",
    )
    .expect("write wrapper");
    fs::write(
        module_dir.join("Cmd.swift"),
        "public struct MyCommand {\n  @Argument var name: String\n  @Argument var count: Int\n}\n",
    )
    .expect("write command");

    let init = run_codegraph(&["init", "--verbose"], dir.path());
    assert!(
        init.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    dir
}

fn mcp_text(result: &codegraph::mcp::protocol::CallToolResult) -> String {
    result
        .content
        .iter()
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn typescript_indexing_builds_call_graph_for_relationship_commands() {
    let dir = fixture_project();

    let callers_output = run_codegraph(&["callers", "helper", "--json"], dir.path());
    assert!(callers_output.status.success(), "stderr:\n{}", stderr(&callers_output));
    let callers: serde_json::Value =
        serde_json::from_str(&stdout(&callers_output)).expect("callers stdout is json");
    assert!(
        callers["callers"]
            .as_array()
            .expect("callers array")
            .iter()
            .any(|node| node["name"] == "runApp"),
        "expected runApp to call helper:\n{callers}"
    );

    let callees_output = run_codegraph(&["callees", "runApp", "--json"], dir.path());
    assert!(callees_output.status.success(), "stderr:\n{}", stderr(&callees_output));
    let callees: serde_json::Value =
        serde_json::from_str(&stdout(&callees_output)).expect("callees stdout is json");
    assert!(
        callees["callees"]
            .as_array()
            .expect("callees array")
            .iter()
            .any(|node| node["name"] == "helper"),
        "expected runApp callees to include helper:\n{callees}"
    );
}

#[test]
fn cpp_indexing_builds_call_graph_for_relationship_commands() {
    let dir = cpp_call_graph_project();

    let callers_output = run_codegraph(&["callers", "helper", "--json"], dir.path());
    assert!(
        callers_output.status.success(),
        "callers failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&callers_output),
        stderr(&callers_output)
    );
    let callers: serde_json::Value =
        serde_json::from_str(&stdout(&callers_output)).expect("callers stdout is json");
    assert!(
        callers["callers"].as_array().expect("callers array").iter().any(|node| node["name"] == "run"),
        "expected run to call helper:\n{callers}"
    );

    let callees_output = run_codegraph(&["callees", "run", "--json"], dir.path());
    assert!(
        callees_output.status.success(),
        "callees failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&callees_output),
        stderr(&callees_output)
    );
    let callees: serde_json::Value =
        serde_json::from_str(&stdout(&callees_output)).expect("callees stdout is json");
    assert!(
        callees["callees"].as_array().expect("callees array").iter().any(|node| node["name"] == "helper"),
        "expected run callees to include helper:\n{callees}"
    );

    let impact_output = run_codegraph(&["impact", "helper", "--json"], dir.path());
    assert!(
        impact_output.status.success(),
        "impact failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&impact_output),
        stderr(&impact_output)
    );
    let impact: serde_json::Value =
        serde_json::from_str(&stdout(&impact_output)).expect("impact stdout is json");
    assert!(
        impact["affected"].as_array().expect("affected array").iter().any(|node| node["name"] == "run"),
        "expected helper impact to include run:\n{impact}"
    );
    assert!(
        impact["edgeCount"].as_u64().unwrap_or(0) > 0,
        "expected impact edgeCount to reflect the call edge:\n{impact}"
    );

    let method_callers_output = run_codegraph(&["callers", "clearIMG", "--json"], dir.path());
    assert!(
        method_callers_output.status.success(),
        "method callers failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&method_callers_output),
        stderr(&method_callers_output)
    );
    let method_callers: serde_json::Value =
        serde_json::from_str(&stdout(&method_callers_output)).expect("method callers stdout is json");
    assert!(
        method_callers["callers"]
            .as_array()
            .expect("callers array")
            .iter()
            .any(|node| node["name"] == "setROIParas"),
        "expected this->clearIMG() to resolve to clearIMG:\n{method_callers}"
    );
}

#[test]
fn python_indexing_builds_call_graph_for_relationship_commands() {
    let dir = python_call_graph_project();

    let callers_output = run_codegraph(&["callers", "helper", "--json"], dir.path());
    assert!(callers_output.status.success(), "stderr:\n{}", stderr(&callers_output));
    let callers: serde_json::Value =
        serde_json::from_str(&stdout(&callers_output)).expect("callers stdout is json");
    assert!(
        callers["callers"]
            .as_array()
            .expect("callers array")
            .iter()
            .any(|node| node["name"] == "run_app"),
        "expected run_app to call helper:\n{callers}"
    );

    let impact_output = run_codegraph(&["impact", "helper", "--json"], dir.path());
    assert!(impact_output.status.success(), "stderr:\n{}", stderr(&impact_output));
    let impact: serde_json::Value =
        serde_json::from_str(&stdout(&impact_output)).expect("impact stdout is json");
    assert!(
        impact["affected"]
            .as_array()
            .expect("affected array")
            .iter()
            .any(|node| node["name"] == "run_app"),
        "expected helper impact to include run_app:\n{impact}"
    );
}

#[test]
fn go_indexing_builds_call_graph_for_relationship_commands() {
    let dir = go_call_graph_project();

    let callers_output = run_codegraph(&["callers", "Helper", "--json"], dir.path());
    assert!(callers_output.status.success(), "stderr:\n{}", stderr(&callers_output));
    let callers: serde_json::Value =
        serde_json::from_str(&stdout(&callers_output)).expect("callers stdout is json");
    assert!(
        callers["callers"]
            .as_array()
            .expect("callers array")
            .iter()
            .any(|node| node["name"] == "Run"),
        "expected Run to call Helper:\n{callers}"
    );

    let callees_output = run_codegraph(&["callees", "Run", "--json"], dir.path());
    assert!(callees_output.status.success(), "stderr:\n{}", stderr(&callees_output));
    let callees: serde_json::Value =
        serde_json::from_str(&stdout(&callees_output)).expect("callees stdout is json");
    assert!(
        callees["callees"]
            .as_array()
            .expect("callees array")
            .iter()
            .any(|node| node["name"] == "Helper"),
        "expected Run callees to include Helper:\n{callees}"
    );

    let impact_output = run_codegraph(&["impact", "Helper", "--json"], dir.path());
    assert!(impact_output.status.success(), "stderr:\n{}", stderr(&impact_output));
    let impact: serde_json::Value =
        serde_json::from_str(&stdout(&impact_output)).expect("impact stdout is json");
    assert!(
        impact["affected"]
            .as_array()
            .expect("affected array")
            .iter()
            .any(|node| node["name"] == "Run"),
        "expected Helper impact to include Run:\n{impact}"
    );
    assert!(
        impact["edgeCount"].as_u64().unwrap_or(0) > 0,
        "expected impact edgeCount to reflect the call edge:\n{impact}"
    );

    let method_callers_output = run_codegraph(&["callers", "Clean", "--json"], dir.path());
    assert!(
        method_callers_output.status.success(),
        "method callers failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&method_callers_output),
        stderr(&method_callers_output)
    );
    let method_callers: serde_json::Value =
        serde_json::from_str(&stdout(&method_callers_output)).expect("method callers stdout is json");
    assert!(
        method_callers["callers"]
            .as_array()
            .expect("callers array")
            .iter()
            .any(|node| node["name"] == "Serve"),
        "expected r.Clean() to resolve to Clean:\n{method_callers}"
    );
}

#[test]
fn java_indexing_builds_call_graph_for_relationship_commands() {
    let dir = java_call_graph_project();

    let callers_output = run_codegraph(&["callers", "Helper", "--json"], dir.path());
    assert!(callers_output.status.success(), "stderr:\n{}", stderr(&callers_output));
    let callers: serde_json::Value =
        serde_json::from_str(&stdout(&callers_output)).expect("callers stdout is json");
    assert!(
        callers["callers"]
            .as_array()
            .expect("callers array")
            .iter()
            .any(|node| node["name"] == "Run"),
        "expected Run to call Helper:\n{callers}"
    );

    let callees_output = run_codegraph(&["callees", "Run", "--json"], dir.path());
    assert!(callees_output.status.success(), "stderr:\n{}", stderr(&callees_output));
    let callees: serde_json::Value =
        serde_json::from_str(&stdout(&callees_output)).expect("callees stdout is json");
    assert!(
        callees["callees"]
            .as_array()
            .expect("callees array")
            .iter()
            .any(|node| node["name"] == "Helper"),
        "expected Run callees to include Helper:\n{callees}"
    );

    let impact_output = run_codegraph(&["impact", "Helper", "--json"], dir.path());
    assert!(impact_output.status.success(), "stderr:\n{}", stderr(&impact_output));
    let impact: serde_json::Value =
        serde_json::from_str(&stdout(&impact_output)).expect("impact stdout is json");
    assert!(
        impact["affected"]
            .as_array()
            .expect("affected array")
            .iter()
            .any(|node| node["name"] == "Run"),
        "expected Helper impact to include Run:\n{impact}"
    );
    assert!(
        impact["edgeCount"].as_u64().unwrap_or(0) > 0,
        "expected impact edgeCount to reflect the call edge:\n{impact}"
    );

    let method_callers_output = run_codegraph(&["callers", "Clean", "--json"], dir.path());
    assert!(
        method_callers_output.status.success(),
        "method callers failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&method_callers_output),
        stderr(&method_callers_output)
    );
    let method_callers: serde_json::Value =
        serde_json::from_str(&stdout(&method_callers_output)).expect("method callers stdout is json");
    assert!(
        method_callers["callers"]
            .as_array()
            .expect("callers array")
            .iter()
            .any(|node| node["name"] == "Serve"),
        "expected this.Clean() to resolve to Clean:\n{method_callers}"
    );
}

#[test]
fn csharp_indexing_builds_call_graph_for_relationship_commands() {
    let dir = csharp_call_graph_project();

    let callers_output = run_codegraph(&["callers", "Helper", "--json"], dir.path());
    assert!(callers_output.status.success(), "stderr:\n{}", stderr(&callers_output));
    let callers: serde_json::Value =
        serde_json::from_str(&stdout(&callers_output)).expect("callers stdout is json");
    assert!(
        callers["callers"]
            .as_array()
            .expect("callers array")
            .iter()
            .any(|node| node["name"] == "Run"),
        "expected Run to call Helper:\n{callers}"
    );

    let callees_output = run_codegraph(&["callees", "Run", "--json"], dir.path());
    assert!(callees_output.status.success(), "stderr:\n{}", stderr(&callees_output));
    let callees: serde_json::Value =
        serde_json::from_str(&stdout(&callees_output)).expect("callees stdout is json");
    assert!(
        callees["callees"]
            .as_array()
            .expect("callees array")
            .iter()
            .any(|node| node["name"] == "Helper"),
        "expected Run callees to include Helper:\n{callees}"
    );

    let impact_output = run_codegraph(&["impact", "Helper", "--json"], dir.path());
    assert!(impact_output.status.success(), "stderr:\n{}", stderr(&impact_output));
    let impact: serde_json::Value =
        serde_json::from_str(&stdout(&impact_output)).expect("impact stdout is json");
    assert!(
        impact["affected"]
            .as_array()
            .expect("affected array")
            .iter()
            .any(|node| node["name"] == "Run"),
        "expected Helper impact to include Run:\n{impact}"
    );
    assert!(
        impact["edgeCount"].as_u64().unwrap_or(0) > 0,
        "expected impact edgeCount to reflect the call edge:\n{impact}"
    );

    let method_callers_output = run_codegraph(&["callers", "Clean", "--json"], dir.path());
    assert!(
        method_callers_output.status.success(),
        "method callers failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&method_callers_output),
        stderr(&method_callers_output)
    );
    let method_callers: serde_json::Value =
        serde_json::from_str(&stdout(&method_callers_output)).expect("method callers stdout is json");
    assert!(
        method_callers["callers"]
            .as_array()
            .expect("callers array")
            .iter()
            .any(|node| node["name"] == "Serve"),
        "expected this.Clean() to resolve to Clean:\n{method_callers}"
    );
}

fn assert_basic_call_graph(dir: &TempDir) {
    let callers_output = run_codegraph(&["callers", "Helper", "--json"], dir.path());
    assert!(callers_output.status.success(), "stderr:\n{}", stderr(&callers_output));
    let callers: serde_json::Value =
        serde_json::from_str(&stdout(&callers_output)).expect("callers stdout is json");
    assert!(
        callers["callers"]
            .as_array()
            .expect("callers array")
            .iter()
            .any(|node| node["name"] == "Run"),
        "expected Run to call Helper:\n{callers}"
    );

    let callees_output = run_codegraph(&["callees", "Run", "--json"], dir.path());
    assert!(callees_output.status.success(), "stderr:\n{}", stderr(&callees_output));
    let callees: serde_json::Value =
        serde_json::from_str(&stdout(&callees_output)).expect("callees stdout is json");
    assert!(
        callees["callees"]
            .as_array()
            .expect("callees array")
            .iter()
            .any(|node| node["name"] == "Helper"),
        "expected Run callees to include Helper:\n{callees}"
    );

    let impact_output = run_codegraph(&["impact", "Helper", "--json"], dir.path());
    assert!(impact_output.status.success(), "stderr:\n{}", stderr(&impact_output));
    let impact: serde_json::Value =
        serde_json::from_str(&stdout(&impact_output)).expect("impact stdout is json");
    assert!(
        impact["affected"]
            .as_array()
            .expect("affected array")
            .iter()
            .any(|node| node["name"] == "Run"),
        "expected Helper impact to include Run:\n{impact}"
    );
    assert!(
        impact["edgeCount"].as_u64().unwrap_or(0) > 0,
        "expected impact edgeCount to reflect the call edge:\n{impact}"
    );

    let method_callers_output = run_codegraph(&["callers", "Clean", "--json"], dir.path());
    assert!(
        method_callers_output.status.success(),
        "method callers failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&method_callers_output),
        stderr(&method_callers_output)
    );
    let method_callers: serde_json::Value =
        serde_json::from_str(&stdout(&method_callers_output)).expect("method callers stdout is json");
    assert!(
        method_callers["callers"]
            .as_array()
            .expect("callers array")
            .iter()
            .any(|node| node["name"] == "Serve"),
        "expected receiver Clean() call to resolve to Clean:\n{method_callers}"
    );
}

#[test]
fn c_like_languages_build_call_graph_for_relationship_commands() {
    let fixtures = [
        (
            "service.php",
            r#"<?php
function Helper($value) {
    return trim($value);
}

function Run($value) {
    return Helper($value);
}

class Service {
    function Clean($value) {
        return $value;
    }

    function Serve($value) {
        return $this->Clean($value);
    }
}
"#,
        ),
        (
            "Service.swift",
            r#"
func Helper(_ value: String) {
}

func Run(_ value: String) {
    Helper(value)
}

class Service {
    func Clean(_ value: String) {
    }

    func Serve(_ value: String) {
        self.Clean(value)
    }
}
"#,
        ),
        (
            "Service.kt",
            r#"
fun Helper(value: String) {
}

fun Run(value: String) {
    Helper(value)
}

class Service {
    fun Clean(value: String) {
    }

    fun Serve(value: String) {
        this.Clean(value)
    }
}
"#,
        ),
        (
            "service.dart",
            r#"
void Helper(String value) {
}

void Run(String value) {
    Helper(value);
}

class Service {
    void Clean(String value) {
    }

    void Serve(String value) {
        this.Clean(value);
    }
}
"#,
        ),
    ];

    for (file_name, source) in fixtures {
        let dir = single_file_call_graph_project(file_name, source);
        assert_basic_call_graph(&dir);
    }
}

#[test]
fn end_block_languages_build_call_graph_for_relationship_commands() {
    let fixtures = [
        (
            "service.rb",
            r#"
def Helper(value)
  value
end

def Run(value)
  Helper(value)
end

def Clean(value)
  value
end

def Serve(value)
  self.Clean(value)
end
"#,
        ),
        (
            "service.lua",
            r#"
function Helper(value)
    return value
end

function Run(value)
    return Helper(value)
end

local Service = {}

function Service.Clean(value)
    return value
end

function Service.Serve(value)
    return Service.Clean(value)
end
"#,
        ),
        (
            "service.luau",
            r#"
function Helper(value: string)
    return value
end

function Run(value: string)
    return Helper(value)
end

local Service = {}

function Service.Clean(value: string)
    return value
end

function Service.Serve(value: string)
    return Service:Clean(value)
end
"#,
        ),
    ];

    for (file_name, source) in fixtures {
        let dir = single_file_call_graph_project(file_name, source);
        assert_basic_call_graph(&dir);
    }
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
            "node", "app.ts", "--file", "app.ts", "--offset", "2", "--limit", "4",
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
    let project =
        codegraph::project::resolve_project(Some(dir.path().to_str().expect("temp path is utf-8")))
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
    let project =
        codegraph::project::resolve_project(Some(dir.path().to_str().expect("temp path is utf-8")))
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
    let rendered = codegraph::query_service::QueryService::render_graph_list(
        "Callees",
        &callees,
        callees.len(),
    );
    let caller_names = callers
        .iter()
        .map(|(node, _)| &node.name)
        .collect::<Vec<_>>();
    let callee_names = callees
        .iter()
        .map(|(node, _)| &node.name)
        .collect::<Vec<_>>();
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
    let output = run_codegraph(&["callers", "helper", "--limit", "1", "--json"], dir.path());
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("callers stdout is json");
    let callers = value["callers"].as_array().expect("callers array");

    assert_eq!(
        callers.len(),
        1,
        "expected limit to apply to emitted callers:\n{value}"
    );
    assert!(
        value["total"].as_u64().unwrap_or(0) >= 3,
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
        out.contains("Callers of 'helper' (3):"),
        "expected header to show full caller count:\n{out}"
    );
    assert_eq!(
        caller_rows, 1,
        "expected limit to apply to visible rows:\n{out}"
    );
    assert!(out.contains("runApp"), "expected visible caller:\n{out}");
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
        &[
            "query",
            "SearchTarget",
            "--kind",
            "class",
            "--limit",
            "1",
            "--json",
        ],
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
    let output = run_codegraph(&["query", "helper", "--kind", "typo", "--json"], dir.path());
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("query stdout is json");
    let results = value["results"].as_array().expect("results array");

    assert!(
        results.is_empty(),
        "invalid kind returned results:\n{value}"
    );
    assert_eq!(value["total"], 0);
}

#[test]
fn explore_returns_source_without_prior_query() {
    let dir = fixture_project();
    let output = run_codegraph(
        &["explore", "runApp helper", "--max-files", "2"],
        dir.path(),
    );
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(
        out.contains("### Sources"),
        "expected sources section:\n{out}"
    );
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
        vec!["codegraph_explore", "codegraph_node", "codegraph_status",]
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
        vec!["codegraph_explore", "codegraph_node", "codegraph_status",]
    );
}

#[test]
fn mcp_default_tools_use_query_service_handlers() {
    let _guard = mcp_tools_env(None);
    let dir = fixture_project();
    let project =
        codegraph::project::resolve_project(Some(dir.path().to_str().expect("temp path is utf-8")))
            .expect("resolve project");
    let db = project.open_database().expect("open database");
    let queries = codegraph::db::QueryBuilder::new(db.get_conn());

    let node = codegraph::mcp::tools::execute_tool(
        "codegraph_node",
        Some(serde_json::json!({"file": "app.ts", "offset": 2, "limit": 3})),
        &project,
        &queries,
    )
    .expect("node tool");
    let search = codegraph::mcp::tools::execute_tool(
        "codegraph_search",
        Some(serde_json::json!({"query": "helper", "kind": "function", "limit": 5})),
        &project,
        &queries,
    )
    .expect("search tool");
    let callers = codegraph::mcp::tools::execute_tool(
        "codegraph_callers",
        Some(serde_json::json!({"symbol": "helper", "limit": 5})),
        &project,
        &queries,
    )
    .expect("callers tool");

    let node_text = mcp_text(&node);
    let search_text = mcp_text(&search);
    let callers_text = mcp_text(&callers);

    assert_ne!(node.is_error, Some(true), "node failed:\n{node_text}");
    assert_ne!(search.is_error, Some(true), "search failed:\n{search_text}");
    assert_ne!(
        callers.is_error,
        Some(true),
        "callers failed:\n{callers_text}"
    );
    assert!(
        node_text.contains("2\t"),
        "expected line numbers:\n{node_text}"
    );
    assert!(
        node_text.contains("helper"),
        "expected file content:\n{node_text}"
    );
    assert!(
        search_text.contains("\"name\": \"helper\""),
        "expected JSON search result:\n{search_text}"
    );
    assert!(
        callers_text.contains("Callers of 'helper'"),
        "expected shared graph renderer:\n{callers_text}"
    );
}

#[test]
fn mcp_allowlisted_status_and_files_are_service_backed() {
    let _guard = mcp_tools_env(Some("status,files"));
    let dir = fixture_project();
    let project =
        codegraph::project::resolve_project(Some(dir.path().to_str().expect("temp path is utf-8")))
            .expect("resolve project");
    let db = project.open_database().expect("open database");
    let queries = codegraph::db::QueryBuilder::new(db.get_conn());

    let status = codegraph::mcp::tools::execute_tool(
        "codegraph_status",
        Some(serde_json::json!({})),
        &project,
        &queries,
    )
    .expect("status tool");
    let files = codegraph::mcp::tools::execute_tool(
        "codegraph_files",
        Some(serde_json::json!({"noMetadata": true})),
        &project,
        &queries,
    )
    .expect("files tool");

    let status_json: serde_json::Value =
        serde_json::from_str(&mcp_text(&status)).expect("status tool returns json");
    let files_json: serde_json::Value =
        serde_json::from_str(&mcp_text(&files)).expect("files tool returns json");
    let app = files_json["files"]
        .as_array()
        .expect("files array")
        .iter()
        .find(|file| file["path"] == "app.ts")
        .expect("app.ts entry");

    assert_ne!(status.is_error, Some(true));
    assert_ne!(files.is_error, Some(true));
    assert_eq!(status_json["initialized"], true);
    assert!(status_json["files"].as_u64().unwrap_or(0) >= 2);
    assert!(
        app.get("language").is_none(),
        "metadata should be omitted:\n{app}"
    );
}

#[test]
fn mcp_staleness_banner_mentions_referenced_pending_file() {
    let _guard = mcp_tools_env(None);
    let dir = fixture_project();
    fs::write(
        dir.path().join("app.ts"),
        "import { helper } from './helper';\nexport function runApp() { return helper() + 10; }\n",
    )
    .expect("edit app without sync");
    let project =
        codegraph::project::resolve_project(Some(dir.path().to_str().expect("temp path is utf-8")))
            .expect("resolve project");
    let db = project.open_database().expect("open database");
    let queries = codegraph::db::QueryBuilder::new(db.get_conn());

    let result = codegraph::mcp::tools::execute_tool(
        "codegraph_search",
        Some(serde_json::json!({"query": "runApp"})),
        &project,
        &queries,
    )
    .expect("search tool");
    let text = mcp_text(&result);

    assert_ne!(result.is_error, Some(true), "search failed:\n{text}");
    assert!(text.starts_with("WARNING:"), "expected stale banner:\n{text}");
    assert!(text.contains("app.ts"), "expected edited file in banner:\n{text}");
    assert!(text.contains("runApp"), "expected original search result:\n{text}");
}

#[test]
fn mcp_staleness_footer_mentions_elsewhere_pending_file() {
    let _guard = mcp_tools_env(None);
    let dir = fixture_project();
    fs::write(
        dir.path().join("src").join("worker.ts"),
        "export function workerMain(name: string) { return name.toLowerCase(); }\n",
    )
    .expect("edit worker without sync");
    let project =
        codegraph::project::resolve_project(Some(dir.path().to_str().expect("temp path is utf-8")))
            .expect("resolve project");
    let db = project.open_database().expect("open database");
    let queries = codegraph::db::QueryBuilder::new(db.get_conn());

    let result = codegraph::mcp::tools::execute_tool(
        "codegraph_search",
        Some(serde_json::json!({"query": "runApp"})),
        &project,
        &queries,
    )
    .expect("search tool");
    let text = mcp_text(&result);

    assert_ne!(result.is_error, Some(true), "search failed:\n{text}");
    assert!(!text.starts_with("WARNING:"), "unexpected banner:\n{text}");
    assert!(
        text.contains("elsewhere in this project are pending index sync"),
        "expected pending footer:\n{text}"
    );
    assert!(
        text.contains("src/worker.ts"),
        "expected edited file in footer:\n{text}"
    );
}

#[test]
fn mcp_staleness_status_json_lists_pending_sync_files() {
    let _guard = mcp_tools_env(Some("status"));
    let dir = fixture_project();
    fs::write(
        dir.path().join("src").join("worker.ts"),
        "export function workerMain(name: string) { return name.toLowerCase(); }\n",
    )
    .expect("edit worker without sync");
    let project =
        codegraph::project::resolve_project(Some(dir.path().to_str().expect("temp path is utf-8")))
            .expect("resolve project");
    let db = project.open_database().expect("open database");
    let queries = codegraph::db::QueryBuilder::new(db.get_conn());

    let result = codegraph::mcp::tools::execute_tool(
        "codegraph_status",
        Some(serde_json::json!({})),
        &project,
        &queries,
    )
    .expect("status tool");
    let status: serde_json::Value =
        serde_json::from_str(&mcp_text(&result)).expect("status tool returns json");

    assert_ne!(result.is_error, Some(true));
    assert_eq!(status["pendingSync"][0]["path"], "src/worker.ts");
    assert!(status["pendingSync"][0]["ageMs"].as_u64().is_some());
}

#[test]
fn mcp_non_default_tool_is_disabled_without_allowlist() {
    let _guard = mcp_tools_env(None);
    let dir = fixture_project();
    let project =
        codegraph::project::resolve_project(Some(dir.path().to_str().expect("temp path is utf-8")))
            .expect("resolve project");
    let db = project.open_database().expect("open database");
    let queries = codegraph::db::QueryBuilder::new(db.get_conn());

    let result = codegraph::mcp::tools::execute_tool(
        "codegraph_files",
        Some(serde_json::json!({})),
        &project,
        &queries,
    )
    .expect("disabled tool result");

    assert_eq!(result.is_error, Some(true));
    assert!(
        mcp_text(&result).contains("disabled via CODEGRAPH_MCP_TOOLS"),
        "unexpected disabled message:\n{}",
        mcp_text(&result)
    );
}

#[test]
fn mcp_missing_required_arguments_return_tool_error() {
    let _guard = mcp_tools_env(None);
    let dir = fixture_project();
    let project = codegraph::project::resolve_project(Some(
        dir.path().to_str().expect("temp path is utf-8"),
    ))
    .expect("resolve project");
    let db = project.open_database().expect("open database");
    let queries = codegraph::db::QueryBuilder::new(db.get_conn());

    let result = codegraph::mcp::tools::execute_tool(
        "codegraph_search",
        Some(serde_json::json!({})),
        &project,
        &queries,
    )
    .expect("missing argument should be a tool-level error");

    assert_eq!(result.is_error, Some(true));
    assert!(
        mcp_text(&result).contains("Missing required argument `query`"),
        "unexpected missing argument message:\n{}",
        mcp_text(&result)
    );
}

#[test]
fn affected_json_matches_typescript_shape_from_stdin() {
    let dir = affected_project();
    let output = run_codegraph_with_input(
        &["affected", "--stdin", "--json", "--depth", "5"],
        dir.path(),
        "src/lib.ts\n",
    );
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("affected stdout is json");

    assert_eq!(value["changedFiles"], serde_json::json!(["src/lib.ts"]));
    assert_eq!(
        value["affectedTests"],
        serde_json::json!(["tests/lib.test.ts"])
    );
    assert_eq!(value["totalDependentsTraversed"], 2);
    assert!(value.get("changed").is_none(), "old key should be absent:\n{value}");
}

#[test]
fn affected_empty_stdin_quiet_exits_zero_without_output() {
    let dir = affected_project();
    let output = run_codegraph_with_input(&["affected", "--stdin", "--quiet"], dir.path(), "");

    assert!(
        output.status.success(),
        "empty stdin should be script-friendly\nstderr:\n{}",
        stderr(&output)
    );
    assert_eq!(stdout(&output), "");
}

#[test]
fn affected_resolves_project_from_subdirectory() {
    let dir = affected_project();
    let src = dir.path().join("src");
    let output = run_codegraph(
        &["affected", "src/lib.ts", "--json", "--depth", "5"],
        &src,
    );
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("affected stdout is json");

    assert_eq!(
        value["affectedTests"],
        serde_json::json!(["tests/lib.test.ts"])
    );
}

#[test]
fn affected_follows_real_typescript_relative_imports() {
    let dir = ts_import_affected_project();
    let output = run_codegraph(&["affected", "src/foo.ts", "--json", "--depth", "5"], dir.path());
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("affected stdout is json");

    assert_eq!(
        value["affectedTests"],
        serde_json::json!(["tests/foo.test.ts"]),
        "expected relative import to make foo.test.ts affected:\n{value}"
    );
}

#[test]
fn affected_follows_typescript_re_export_imports() {
    let dir = ts_reexport_affected_project();
    let output = run_codegraph(&["affected", "src/foo.ts", "--json", "--depth", "5"], dir.path());
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("affected stdout is json");

    assert_eq!(
        value["affectedTests"],
        serde_json::json!(["tests/foo.test.ts"]),
        "expected re-export chain to make foo.test.ts affected:\n{value}"
    );
}

#[test]
fn affected_follows_python_imported_symbol_without_calling_it() {
    let dir = python_import_affected_project();
    let output = run_codegraph(&["affected", "pkg/foo.py", "--json", "--depth", "5"], dir.path());
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("affected stdout is json");

    assert_eq!(
        value["affectedTests"],
        serde_json::json!(["pkg/foo.test.py"]),
        "expected import-from symbol to make foo.test.py affected:\n{value}"
    );
}

#[test]
fn affected_follows_python_relative_module_imports() {
    let dir = python_relative_module_affected_project();
    let output = run_codegraph(&["affected", "pkg/certs.py", "--json", "--depth", "5"], dir.path());
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("affected stdout is json");

    assert_eq!(
        value["affectedTests"],
        serde_json::json!(["pkg/utils.test.py"]),
        "expected relative module import to make utils.test.py affected:\n{value}"
    );
}

#[test]
fn affected_follows_rust_pub_use_reexport_hub() {
    let dir = rust_use_reexport_project();
    let output = run_codegraph(
        &["affected", "src/api/widget.rs", "--json", "--depth", "5", "--filter", "src/api/mod.rs"],
        dir.path(),
    );
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("affected stdout is json");

    assert_eq!(
        value["affectedTests"],
        serde_json::json!(["src/api/mod.rs"]),
        "expected pub use hub to depend on widget.rs:\n{value}"
    );
}

#[test]
fn affected_resolves_rust_pub_use_to_qualified_module_path() {
    let dir = rust_use_collision_project();
    let fast = run_codegraph(
        &["affected", "src/fast.rs", "--json", "--depth", "5", "--filter", "src/hub.rs"],
        dir.path(),
    );
    assert!(fast.status.success(), "stderr:\n{}", stderr(&fast));
    let fast_value: serde_json::Value =
        serde_json::from_str(&stdout(&fast)).expect("affected stdout is json");
    assert_eq!(
        fast_value["affectedTests"],
        serde_json::json!(["src/hub.rs"]),
        "expected hub.rs to depend on fast.rs:\n{fast_value}"
    );

    let slow = run_codegraph(
        &["affected", "src/slow.rs", "--json", "--depth", "5", "--filter", "src/hub.rs"],
        dir.path(),
    );
    assert!(slow.status.success(), "stderr:\n{}", stderr(&slow));
    let slow_value: serde_json::Value =
        serde_json::from_str(&stdout(&slow)).expect("affected stdout is json");
    assert_eq!(
        slow_value["affectedTests"],
        serde_json::json!([]),
        "hub.rs must not depend on slow.rs:\n{slow_value}"
    );
}

#[test]
fn affected_follows_java_annotation_usage() {
    let dir = java_annotation_project();
    let output = run_codegraph(
        &["affected", "p/MyAnno.java", "--json", "--depth", "5", "--filter", "p/User.java"],
        dir.path(),
    );
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("affected stdout is json");

    assert_eq!(
        value["affectedTests"],
        serde_json::json!(["p/User.java"]),
        "expected annotation usage to make User.java affected:\n{value}"
    );
}

#[test]
fn affected_follows_swift_property_wrapper_usage() {
    let dir = swift_property_wrapper_project();
    let output = run_codegraph(
        &[
            "affected",
            "Sources/M/Wrap.swift",
            "--json",
            "--depth",
            "5",
            "--filter",
            "Sources/M/Cmd.swift",
        ],
        dir.path(),
    );
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("affected stdout is json");

    assert_eq!(
        value["affectedTests"],
        serde_json::json!(["Sources/M/Cmd.swift"]),
        "expected @Argument usage to make Cmd.swift affected:\n{value}"
    );
}

#[test]
fn affected_follows_csharp_record_references() {
    let dir = csharp_record_project();
    let output = run_codegraph(
        &["affected", "types.cs", "--json", "--depth", "5", "--filter", "use.cs"],
        dir.path(),
    );
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("affected stdout is json");

    assert_eq!(
        value["affectedTests"],
        serde_json::json!(["use.cs"]),
        "expected record references to make use.cs affected:\n{value}"
    );
}

#[test]
fn affected_follows_go_cross_package_composite_literal() {
    let dir = go_composite_literal_project();
    let output = run_codegraph(
        &["affected", "render/xml.go", "--json", "--depth", "5", "--filter", "app.go"],
        dir.path(),
    );
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("affected stdout is json");

    assert_eq!(
        value["affectedTests"],
        serde_json::json!(["app.go"]),
        "expected render.XML composite literal to make app.go affected:\n{value}"
    );
}

#[test]
fn affected_follows_go_package_level_composite_literal() {
    let dir = go_composite_registry_project();
    let output = run_codegraph(
        &["affected", "render/xml.go", "--json", "--depth", "5", "--filter", "reg.go"],
        dir.path(),
    );
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("affected stdout is json");

    assert_eq!(
        value["affectedTests"],
        serde_json::json!(["reg.go"]),
        "expected registry render.XML literal to make reg.go affected:\n{value}"
    );
}

#[test]
fn affected_follows_go_parenthesized_pointer_conversion() {
    let dir = go_pointer_conversion_project();
    let output = run_codegraph(
        &["affected", "types.go", "--json", "--depth", "5", "--filter", "use.go"],
        dir.path(),
    );
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("affected stdout is json");

    assert_eq!(
        value["affectedTests"],
        serde_json::json!(["use.go"]),
        "expected (*Wrapped)(x) conversion to make use.go affected:\n{value}"
    );
}

#[test]
fn go_top_level_closure_call_is_attributed_to_variable() {
    let dir = go_top_level_closure_project();
    let output = run_codegraph(&["callers", "Wire", "--json"], dir.path());
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("callers stdout is json");
    let callers = value["callers"].as_array().expect("callers array");

    assert!(
        callers
            .iter()
            .any(|node| node["kind"] == "variable" && node["name"] == "rootCmd"),
        "expected rootCmd variable to call Wire:\n{value}"
    );
    assert!(
        callers.iter().all(|node| node["kind"] != "file"),
        "closure call must not be attributed to a file node:\n{value}"
    );
}

#[test]
fn affected_follows_go_implicit_interface_implementation() {
    let dir = go_implicit_interface_project();
    let output = run_codegraph(
        &["affected", "codec/json.go", "--json", "--depth", "5", "--filter", "codec/api.go"],
        dir.path(),
    );
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("affected stdout is json");

    assert_eq!(
        value["affectedTests"],
        serde_json::json!(["codec/api.go"]),
        "expected Core interface to depend on jsonApi implementation:\n{value}"
    );
}

#[test]
fn install_print_config_codex_writes_no_files() {
    let dir = tempfile::tempdir().expect("temp install project");
    let home = dir.path().join("home");
    fs::create_dir_all(&home).expect("create home");

    let output = run_codegraph_with_env(
        &["install", "--print-config", "codex", "--location", "global"],
        dir.path(),
        &[("HOME", &home), ("USERPROFILE", &home)],
    );

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(
        out.contains("[mcp_servers.codegraph]"),
        "expected codex toml config:\n{out}"
    );
    assert!(
        out.contains("command = \"codegraph\""),
        "expected codegraph command:\n{out}"
    );
    assert!(
        !home.join(".codex").join("config.toml").exists(),
        "print-config must not write config files"
    );
}

#[test]
fn install_and_uninstall_cursor_local_updates_mcp_json() {
    let dir = tempfile::tempdir().expect("temp install project");
    let home = dir.path().join("home");
    fs::create_dir_all(&home).expect("create home");

    let install = run_codegraph_with_env(
        &[
            "install",
            "--target",
            "cursor",
            "--location",
            "local",
            "--yes",
        ],
        dir.path(),
        &[("HOME", &home), ("USERPROFILE", &home)],
    );
    assert!(install.status.success(), "stderr:\n{}", stderr(&install));

    let mcp_path = dir.path().join(".cursor").join("mcp.json");
    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&mcp_path).expect("cursor mcp config"))
            .expect("cursor config json");
    let server = &config["mcpServers"]["codegraph"];
    assert_eq!(server["type"], "stdio");
    assert_eq!(server["command"], "codegraph");
    assert!(
        server["args"]
            .as_array()
            .expect("args array")
            .iter()
            .any(|arg| arg == "--path"),
        "cursor local config should include --path:\n{config}"
    );

    let uninstall = run_codegraph_with_env(
        &[
            "uninstall",
            "--target",
            "cursor",
            "--location",
            "local",
            "--yes",
        ],
        dir.path(),
        &[("HOME", &home), ("USERPROFILE", &home)],
    );
    assert!(uninstall.status.success(), "stderr:\n{}", stderr(&uninstall));
    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&mcp_path).expect("cursor mcp config"))
            .expect("cursor config json");
    assert!(
        config["mcpServers"].get("codegraph").is_none(),
        "uninstall should remove codegraph entry:\n{config}"
    );
}
