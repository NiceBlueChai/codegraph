//! Launcher-side MCP daemon proxy.
//!
//! The proxy keeps the CLI process as the client's stdio endpoint while the
//! project-scoped daemon owns the reusable MCP service.

use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{Shutdown, TcpStream};
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

use crate::mcp::daemon_paths::{
    daemon_pid_path, daemon_starting_lock_path, pid_is_alive, read_daemon_lock,
};
use crate::project::ProjectContext;

/// Connects stdio to the project daemon, spawning it if needed.
pub fn run(project: ProjectContext) -> anyhow::Result<()> {
    let stream = connect_or_spawn(&project)?;
    proxy_stdio(stream)
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

    let result = spawn_daemon(project).and_then(|_| wait_for_daemon(project, Duration::from_secs(5)));
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

fn proxy_stdio(stream: TcpStream) -> anyhow::Result<()> {
    let mut daemon_writer = stream.try_clone()?;
    let stdin_done = Arc::new(AtomicBool::new(false));
    let stdin_done_for_thread = stdin_done.clone();
    let stdin_thread = thread::spawn(move || -> io::Result<()> {
        let mut stdin = io::stdin().lock();
        io::copy(&mut stdin, &mut daemon_writer)?;
        daemon_writer.shutdown(Shutdown::Write)?;
        stdin_done_for_thread.store(true, Ordering::SeqCst);
        Ok(())
    });

    stream.set_read_timeout(Some(Duration::from_millis(100)))?;
    let mut daemon_reader = BufReader::new(stream);
    let mut stdout = io::stdout().lock();
    loop {
        let mut line = String::new();
        match daemon_reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                stdout.write_all(line.as_bytes())?;
                stdout.flush()?;
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                if stdin_done.load(Ordering::SeqCst) {
                    break;
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
    stdout.flush()?;
    stdin_thread
        .join()
        .map_err(|_| anyhow::anyhow!("stdin proxy thread panicked"))??;
    Ok(())
}
