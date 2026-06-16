//! Process-level tests for the Rust MCP daemon/proxy runtime.
//!
//! These tests exercise the real `codegraph` binary because daemon behavior is
//! mostly process coordination, not pure library logic.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use tempfile::TempDir;

fn codegraph_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_codegraph"))
}

fn fixture_project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    fs::write(
        dir.path().join("app.ts"),
        "export function runApp() { return 1; }\n",
    )
    .expect("write fixture");
    let output = Command::new(codegraph_bin())
        .arg("init")
        .current_dir(dir.path())
        .output()
        .expect("init command starts");
    assert!(
        output.status.success(),
        "init failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    dir
}

fn spawn_daemon(project: &Path) -> Child {
    Command::new(codegraph_bin())
        .args(["serve", "--mcp-daemon", "--path"])
        .arg(project)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn daemon")
}

fn read_pidfile(project: &Path) -> serde_json::Value {
    let path = project.join(".codegraph").join("daemon.pid");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(text) = fs::read_to_string(&path) {
            return serde_json::from_str(&text).expect("daemon pidfile json");
        }
        if Instant::now() > deadline {
            panic!("timed out waiting for {}", path.display());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn wait_for_pidfile_pid_not(project: &Path, stale_pid: u64) -> serde_json::Value {
    let path = project.join(".codegraph").join("daemon.pid");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(text) = fs::read_to_string(&path) {
            let value: serde_json::Value =
                serde_json::from_str(&text).expect("daemon pidfile json");
            if value["pid"].as_u64() != Some(stale_pid) {
                return value;
            }
        }
        if Instant::now() > deadline {
            panic!(
                "timed out waiting for pidfile replacement: {}",
                path.display()
            );
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn wait_for_pidfile_created(project: &Path) {
    let path = project.join(".codegraph").join("daemon.pid");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if path.exists() {
            return;
        }
        if Instant::now() > deadline {
            panic!("timed out waiting for {}", path.display());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn wait_for_pidfile_removed(project: &Path) {
    let path = project.join(".codegraph").join("daemon.pid");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if !path.exists() {
            return;
        }
        if Instant::now() > deadline {
            panic!("timed out waiting for pidfile removal: {}", path.display());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn initialize_over_tcp(addr: &str) -> serde_json::Value {
    let stream = TcpStream::connect(addr).expect("connect daemon");
    let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
    let mut writer = stream;
    let mut hello = String::new();
    reader.read_line(&mut hello).expect("read daemon hello");
    assert!(
        hello.contains("\"codegraph\""),
        "unexpected daemon hello: {hello}"
    );

    writeln!(
        writer,
        "{}",
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "daemon-test", "version": "0.0.0"}
            }
        })
    )
    .expect("write initialize");
    writer.flush().expect("flush initialize");

    let mut response = String::new();
    reader
        .read_line(&mut response)
        .expect("read initialize response");
    serde_json::from_str(&response).expect("initialize response json")
}

fn initialize_over_stdio(project: &Path) -> serde_json::Value {
    initialize_over_stdio_with_env(project, &[])
}

fn initialize_over_stdio_with_env(project: &Path, envs: &[(&str, &str)]) -> serde_json::Value {
    let daemon_disabled = envs.iter().any(|(key, value)| {
        *key == "CODEGRAPH_NO_DAEMON"
            && !value.is_empty()
            && *value != "0"
            && !value.eq_ignore_ascii_case("false")
    });
    let pidfile_existed = project.join(".codegraph").join("daemon.pid").exists();
    let input = format!(
        "{}\n{}\n",
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "proxy-test", "version": "0.0.0"}
            }
        }),
        serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
    );
    let mut command = Command::new(codegraph_bin());
    command
        .args(["serve", "--mcp", "--path"])
        .arg(project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in envs {
        command.env(key, value);
    }
    let mut child = command.spawn().expect("spawn mcp proxy");
    let stdout = child.stdout.take().expect("stdout");
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let result = reader.read_line(&mut line).map(|_| line);
        let _ = tx.send(result);
    });

    let mut stdin = child.stdin.take().expect("stdin");
    stdin.write_all(input.as_bytes()).expect("write stdin");
    stdin.flush().expect("flush stdin");
    drop(stdin);
    let line = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("read initialize response before timeout")
        .expect("read initialize response");
    if !daemon_disabled && !pidfile_existed {
        wait_for_pidfile_created(project);
    }
    stop_child(child);
    serde_json::from_str(&line).expect("initialize response json")
}

fn request_over_stdio(
    project: &Path,
    messages: Vec<serde_json::Value>,
    ids: &[i64],
    timeout: Duration,
) -> (HashMap<i64, serde_json::Value>, Child) {
    let mut child = Command::new(codegraph_bin())
        .args(["serve", "--mcp", "--path"])
        .arg(project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn mcp proxy");

    let stdout = child.stdout.take().expect("stdout");
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let _ = tx.send(line);
                }
                Err(_) => break,
            }
        }
    });

    let mut stdin = child.stdin.take().expect("stdin");
    for message in messages {
        writeln!(stdin, "{message}").expect("write message");
    }
    stdin.flush().expect("flush stdin");

    let deadline = Instant::now() + timeout;
    let mut responses = HashMap::new();
    while responses.len() < ids.len() {
        let now = Instant::now();
        assert!(now < deadline, "timed out waiting for MCP responses");
        let line = rx
            .recv_timeout(deadline.saturating_duration_since(now))
            .expect("read MCP response before timeout");
        let value: serde_json::Value = serde_json::from_str(&line).expect("MCP response json");
        if let Some(id) = value["id"].as_i64() {
            if ids.contains(&id) {
                responses.insert(id, value);
            }
        }
    }

    (responses, child)
}

fn stop_child(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn stop_pid(pid: &serde_json::Value) {
    if let Some(pid) = pid.as_u64() {
        if pid == u64::from(std::process::id()) {
            return;
        }

        #[cfg(windows)]
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output();

        #[cfg(unix)]
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGKILL);
        }
    }
}

fn write_pidfile(project: &Path, pid: u32, version: &str, addr: &str) {
    let path = project.join(".codegraph").join("daemon.pid");
    fs::write(
        path,
        serde_json::json!({
            "pid": pid,
            "version": version,
            "addr": addr,
            "startedAt": 1,
        })
        .to_string(),
    )
    .expect("write pidfile");
}

#[test]
fn hidden_daemon_mode_answers_initialize() {
    let project = fixture_project();
    let daemon = spawn_daemon(project.path());
    let pidfile = read_pidfile(project.path());
    let addr = pidfile["addr"].as_str().expect("pidfile addr");

    let response = initialize_over_tcp(addr);

    stop_child(daemon);
    assert_eq!(response["result"]["serverInfo"]["name"], "CodeGraph");
}

#[test]
fn two_mcp_launchers_share_one_daemon() {
    let project = fixture_project();

    let first = initialize_over_stdio(project.path());
    assert_eq!(first["result"]["serverInfo"]["name"], "CodeGraph");
    let first_pid = read_pidfile(project.path())["pid"].clone();

    let second = initialize_over_stdio(project.path());
    assert_eq!(second["result"]["serverInfo"]["name"], "CodeGraph");
    let second_pid = read_pidfile(project.path())["pid"].clone();

    stop_pid(&second_pid);
    assert_eq!(first_pid, second_pid);
}

#[test]
fn no_daemon_env_uses_direct_mode_without_pidfile() {
    let project = fixture_project();

    let response =
        initialize_over_stdio_with_env(project.path(), &[("CODEGRAPH_NO_DAEMON", "true")]);

    assert_eq!(response["result"]["serverInfo"]["name"], "CodeGraph");
    assert!(!project
        .path()
        .join(".codegraph")
        .join("daemon.pid")
        .exists());
}

#[test]
fn no_daemon_false_value_keeps_daemon_enabled() {
    let project = fixture_project();

    let response =
        initialize_over_stdio_with_env(project.path(), &[("CODEGRAPH_NO_DAEMON", "false")]);
    let pid = read_pidfile(project.path())["pid"].clone();

    stop_pid(&pid);
    assert_eq!(response["result"]["serverInfo"]["name"], "CodeGraph");
}

#[test]
fn stale_pidfile_is_replaced() {
    let project = fixture_project();
    write_pidfile(
        project.path(),
        999_999,
        env!("CARGO_PKG_VERSION"),
        "127.0.0.1:1",
    );

    let messages = vec![serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "proxy-test", "version": "0.0.0"}
        }
    })];
    let (responses, child) =
        request_over_stdio(project.path(), messages, &[1], Duration::from_secs(5));
    let pid = wait_for_pidfile_pid_not(project.path(), 999_999)["pid"].clone();

    stop_child(child);
    stop_pid(&pid);
    assert_eq!(responses[&1]["result"]["serverInfo"]["name"], "CodeGraph");
    assert_ne!(pid, serde_json::json!(999_999));
}

#[test]
fn version_mismatch_falls_back_to_direct_mode() {
    let project = fixture_project();
    write_pidfile(
        project.path(),
        std::process::id(),
        "0.0.0-old",
        "127.0.0.1:1",
    );

    let response = initialize_over_stdio(project.path());
    let pidfile = read_pidfile(project.path());

    assert_eq!(response["result"]["serverInfo"]["name"], "CodeGraph");
    assert_eq!(pidfile["version"], "0.0.0-old");
}

#[test]
fn local_handshake_answers_initialize_and_tools_while_daemon_start_is_locked() {
    let project = fixture_project();
    let lock_path = project
        .path()
        .join(".codegraph")
        .join("daemon.starting.lock");
    let lock = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .expect("hold daemon startup lock");

    let messages = vec![
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "proxy-test", "version": "0.0.0"}
            }
        }),
        serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
    ];

    let (responses, child) =
        request_over_stdio(project.path(), messages, &[1, 2], Duration::from_secs(1));

    stop_child(child);
    drop(lock);
    let _ = fs::remove_file(lock_path);
    assert_eq!(responses[&1]["result"]["serverInfo"]["name"], "CodeGraph");
    assert!(responses[&2]["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .any(|tool| tool["name"] == "codegraph_explore"));
}

#[test]
fn local_handshake_answers_empty_resource_and_prompt_lists() {
    let project = fixture_project();

    let messages = vec![
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "proxy-test", "version": "0.0.0"}
            }
        }),
        serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "resources/list", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "resources/templates/list",
            "params": {}
        }),
        serde_json::json!({"jsonrpc": "2.0", "id": 4, "method": "prompts/list", "params": {}}),
    ];

    let (responses, child) =
        request_over_stdio(project.path(), messages, &[2, 3, 4], Duration::from_secs(5));
    let pid = read_pidfile(project.path())["pid"].clone();

    stop_child(child);
    stop_pid(&pid);
    assert_eq!(responses[&2]["result"]["resources"], serde_json::json!([]));
    assert_eq!(
        responses[&3]["result"]["resourceTemplates"],
        serde_json::json!([])
    );
    assert_eq!(responses[&4]["result"]["prompts"], serde_json::json!([]));
}

#[test]
fn daemon_idle_timeout_removes_pidfile() {
    let project = fixture_project();

    let response = initialize_over_stdio_with_env(
        project.path(),
        &[("CODEGRAPH_DAEMON_IDLE_TIMEOUT_MS", "200")],
    );

    assert_eq!(response["result"]["serverInfo"]["name"], "CodeGraph");
    wait_for_pidfile_removed(project.path());
}
