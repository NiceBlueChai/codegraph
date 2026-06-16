//! Process-level tests for the Rust MCP daemon/proxy runtime.
//!
//! These tests exercise the real `codegraph` binary because daemon behavior is
//! mostly process coordination, not pure library logic.

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
    stop_child(child);
    serde_json::from_str(&line).expect("initialize response json")
}

fn stop_child(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn stop_pid(pid: &serde_json::Value) {
    if let Some(pid) = pid.as_u64() {
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
