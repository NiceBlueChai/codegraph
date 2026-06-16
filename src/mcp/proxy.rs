//! Launcher-side MCP daemon proxy.
//!
//! The proxy keeps the CLI process as the client's stdio endpoint while the
//! project-scoped daemon owns the reusable MCP service.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpStream};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::mcp::daemon_paths::{
    daemon_pid_path, daemon_starting_lock_path, pid_is_alive, read_daemon_lock,
};
use crate::mcp::tools;
use crate::project::ProjectContext;
use serde_json::{json, Value};

/// Connects stdio to the project daemon, spawning it if needed.
pub fn run(project: ProjectContext) -> anyhow::Result<()> {
    local_handshake_proxy(project)
}

fn connect_or_spawn(project: &ProjectContext) -> anyhow::Result<TcpStream> {
    if let Ok(info) = read_daemon_lock(daemon_pid_path(&project.root)) {
        if info.version != env!("CARGO_PKG_VERSION") && pid_is_alive(info.pid) {
            anyhow::bail!("daemon version mismatch");
        }
    }

    if let Ok(stream) = connect_existing(project) {
        return Ok(stream);
    }

    let lock_path = daemon_starting_lock_path(&project.root);
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let lock = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path);

    if lock.is_err() {
        return wait_for_daemon(project, Duration::from_secs(5));
    }

    let result =
        spawn_daemon(project).and_then(|_| wait_for_daemon(project, Duration::from_secs(5)));
    let _ = fs::remove_file(lock_path);
    result
}

fn connect_existing(project: &ProjectContext) -> anyhow::Result<TcpStream> {
    let info = read_daemon_lock(daemon_pid_path(&project.root))?;
    if info.version != env!("CARGO_PKG_VERSION") {
        anyhow::bail!("daemon version mismatch");
    }
    if !pid_is_alive(info.pid) {
        let _ = fs::remove_file(daemon_pid_path(&project.root));
        anyhow::bail!("stale daemon pid");
    }
    connect_with_hello(&info.addr)
}

fn wait_for_daemon(project: &ProjectContext, timeout: Duration) -> anyhow::Result<TcpStream> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(stream) = connect_existing(project) {
            return Ok(stream);
        }
        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for daemon");
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn spawn_daemon(project: &ProjectContext) -> anyhow::Result<()> {
    Command::new(std::env::current_exe()?)
        .args(["serve", "--mcp-daemon", "--path"])
        .arg(&project.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

fn connect_with_hello(addr: &str) -> anyhow::Result<TcpStream> {
    let stream = TcpStream::connect(addr)?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut hello = String::new();
    reader.read_line(&mut hello)?;
    if !hello.contains("\"codegraph\"") {
        anyhow::bail!("invalid daemon hello");
    }
    Ok(stream)
}

fn local_handshake_proxy(project: ProjectContext) -> anyhow::Result<()> {
    let (stdin_tx, stdin_rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(line) => {
                    if stdin_tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let (connect_tx, connect_rx) = mpsc::channel::<anyhow::Result<TcpStream>>();
    let project_for_connect = project.clone();
    thread::spawn(move || {
        let _ = connect_tx.send(connect_or_spawn(&project_for_connect));
    });

    let mut daemon_writer: Option<TcpStream> = None;
    let mut daemon_failed = false;
    let mut daemon_input_closed = false;
    let mut daemon_reader_done = false;
    let mut stdin_closed = false;
    let mut pending = Vec::<String>::new();
    let mut local_initialize_id: Option<Value> = None;
    let (daemon_tx, daemon_rx) = mpsc::channel::<String>();
    let mut stdout = std::io::stdout().lock();

    loop {
        if daemon_writer.is_none() && !daemon_failed {
            match connect_rx.try_recv() {
                Ok(Ok(stream)) => {
                    let reader_stream = stream.try_clone()?;
                    spawn_daemon_reader(reader_stream, daemon_tx.clone());
                    daemon_writer = Some(stream);
                    flush_pending(&mut pending, daemon_writer.as_mut())?;
                }
                Ok(Err(error)) => {
                    log::debug!("Shared daemon unavailable; serving this session locally: {error}");
                    daemon_failed = true;
                    serve_pending_locally(&project, &mut pending, &mut stdout)?;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    daemon_failed = true;
                    serve_pending_locally(&project, &mut pending, &mut stdout)?;
                }
            }
        }

        loop {
            match daemon_rx.try_recv() {
                Ok(line) => {
                    if should_suppress_daemon_response(&line, local_initialize_id.as_ref()) {
                        continue;
                    }
                    writeln!(stdout, "{line}")?;
                    stdout.flush()?;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    daemon_reader_done = true;
                    break;
                }
            }
        }

        match stdin_rx.recv_timeout(Duration::from_millis(10)) {
            Ok(line) => {
                if line.trim().is_empty() {
                    continue;
                }
                handle_client_line(
                    &project,
                    &line,
                    &mut local_initialize_id,
                    daemon_writer.as_mut(),
                    daemon_failed,
                    &mut pending,
                    &mut stdout,
                )?;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                stdin_closed = true;
            }
        }

        if stdin_closed {
            if daemon_writer.is_none() && !daemon_failed {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            if let Some(writer) = daemon_writer.as_mut() {
                if !daemon_input_closed {
                    let _ = writer.shutdown(Shutdown::Write);
                    daemon_input_closed = true;
                }
            }
            if daemon_failed || daemon_reader_done || daemon_writer.is_none() {
                break;
            }
        }
    }

    stdout.flush()?;
    Ok(())
}

fn spawn_daemon_reader(stream: TcpStream, daemon_tx: mpsc::Sender<String>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let line = line.trim_end_matches(['\r', '\n']).to_string();
                    if daemon_tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

fn handle_client_line(
    project: &ProjectContext,
    line: &str,
    local_initialize_id: &mut Option<Value>,
    daemon_writer: Option<&mut TcpStream>,
    daemon_failed: bool,
    pending: &mut Vec<String>,
    stdout: &mut impl Write,
) -> anyhow::Result<()> {
    let parsed = serde_json::from_str::<Value>(line).ok();
    if let Some(message) = parsed.as_ref() {
        match message["method"].as_str() {
            Some("initialize") => {
                *local_initialize_id = message.get("id").cloned();
                write_result(
                    stdout,
                    message.get("id").cloned(),
                    local_initialize_result(),
                )?;
                route_line(line, daemon_writer, daemon_failed, pending, project, stdout)?;
                return Ok(());
            }
            Some("tools/list") => {
                write_result(
                    stdout,
                    message.get("id").cloned(),
                    json!({ "tools": tools::register_tools_json() }),
                )?;
                return Ok(());
            }
            Some("resources/list") => {
                write_result(
                    stdout,
                    message.get("id").cloned(),
                    json!({ "resources": [] }),
                )?;
                return Ok(());
            }
            Some("resources/templates/list") => {
                write_result(
                    stdout,
                    message.get("id").cloned(),
                    json!({ "resourceTemplates": [] }),
                )?;
                return Ok(());
            }
            Some("prompts/list") => {
                write_result(stdout, message.get("id").cloned(), json!({ "prompts": [] }))?;
                return Ok(());
            }
            _ => {}
        }
    }

    route_line(line, daemon_writer, daemon_failed, pending, project, stdout)
}

fn route_line(
    line: &str,
    daemon_writer: Option<&mut TcpStream>,
    daemon_failed: bool,
    pending: &mut Vec<String>,
    project: &ProjectContext,
    stdout: &mut impl Write,
) -> anyhow::Result<()> {
    if let Some(writer) = daemon_writer {
        writeln!(writer, "{line}")?;
        writer.flush()?;
    } else if daemon_failed {
        handle_locally(project, line, stdout)?;
    } else {
        pending.push(line.to_string());
    }
    Ok(())
}

fn flush_pending(
    pending: &mut Vec<String>,
    daemon_writer: Option<&mut TcpStream>,
) -> anyhow::Result<()> {
    let Some(writer) = daemon_writer else {
        return Ok(());
    };
    for line in pending.drain(..) {
        writeln!(writer, "{line}")?;
    }
    writer.flush()?;
    Ok(())
}

fn serve_pending_locally(
    project: &ProjectContext,
    pending: &mut Vec<String>,
    stdout: &mut impl Write,
) -> anyhow::Result<()> {
    for line in pending.drain(..) {
        handle_locally(project, &line, stdout)?;
    }
    Ok(())
}

fn handle_locally(
    project: &ProjectContext,
    line: &str,
    stdout: &mut impl Write,
) -> anyhow::Result<()> {
    let Ok(message) = serde_json::from_str::<Value>(line) else {
        return Ok(());
    };
    let id = message.get("id").cloned();
    match message["method"].as_str() {
        Some("tools/call") => handle_tool_call_locally(project, &message, stdout),
        Some("ping") => write_result(stdout, id, json!({})),
        Some("initialize") | Some("notifications/initialized") => Ok(()),
        Some(_) if id.is_some() => write_error(stdout, id, -32603, "CodeGraph daemon unavailable"),
        _ => Ok(()),
    }
}

fn handle_tool_call_locally(
    project: &ProjectContext,
    message: &Value,
    stdout: &mut impl Write,
) -> anyhow::Result<()> {
    let id = message.get("id").cloned();
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return write_error(stdout, id, -32602, "tools/call missing params.name");
    };
    let arguments = params.get("arguments").cloned();
    let db = project.open_database()?;
    let queries = crate::db::QueryBuilder::new(db.get_conn());
    match tools::execute_tool(name, arguments, project, &queries) {
        Ok(result) => {
            let mut value = serde_json::to_value(result)?;
            if let Some(object) = value.as_object_mut() {
                if let Some(is_error) = object.remove("is_error") {
                    object.insert("isError".to_string(), is_error);
                }
            }
            write_result(stdout, id, value)
        }
        Err(error) => write_error(stdout, id, -32603, &error.to_string()),
    }
}

fn write_result(stdout: &mut impl Write, id: Option<Value>, result: Value) -> anyhow::Result<()> {
    writeln!(
        stdout,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        })
    )?;
    stdout.flush()?;
    Ok(())
}

fn write_error(
    stdout: &mut impl Write,
    id: Option<Value>,
    code: i64,
    message: &str,
) -> anyhow::Result<()> {
    writeln!(
        stdout,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": code,
                "message": message,
            },
        })
    )?;
    stdout.flush()?;
    Ok(())
}

fn local_initialize_result() -> Value {
    json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {},
        },
        "serverInfo": {
            "name": "CodeGraph",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": tools::server_instructions(true),
    })
}

fn should_suppress_daemon_response(line: &str, local_initialize_id: Option<&Value>) -> bool {
    let Some(local_initialize_id) = local_initialize_id else {
        return false;
    };
    let Ok(message) = serde_json::from_str::<Value>(line) else {
        return false;
    };
    message.get("id") == Some(local_initialize_id)
        && (message.get("result").is_some() || message.get("error").is_some())
}
