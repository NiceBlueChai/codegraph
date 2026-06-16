//! Project-scoped MCP daemon listener.
//!
//! The daemon owns the long-lived TCP listener. Each accepted connection is
//! handed to the same rmcp-backed service used by direct stdio mode.

use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::mcp::daemon_paths::{daemon_pid_path, write_daemon_lock, DaemonLockInfo};
use crate::mcp::service::CodeGraphMcpService;
use crate::project::ProjectContext;

/// Runs the MCP daemon for one initialized project until the process exits.
pub fn run_daemon(project: ProjectContext) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run_daemon_async(project))
}

async fn run_daemon_async(project: ProjectContext) -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?.to_string();
    let pid_path = daemon_pid_path(&project.root);
    let info = DaemonLockInfo {
        pid: std::process::id(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        addr: addr.clone(),
        started_at: chrono::Utc::now().timestamp_millis(),
    };
    write_daemon_lock(&pid_path, &info)?;
    let service = CodeGraphMcpService::active(project);
    let idle_timeout = daemon_idle_timeout();
    let (done_tx, mut done_rx) = mpsc::unbounded_channel::<()>();
    let mut active_connections = 0usize;

    loop {
        let accepted = if active_connections == 0 {
            tokio::select! {
                accepted = listener.accept() => accepted,
                _ = tokio::time::sleep(idle_timeout) => break,
            }
        } else {
            tokio::select! {
                accepted = listener.accept() => accepted,
                Some(_) = done_rx.recv() => {
                    active_connections = active_connections.saturating_sub(1);
                    continue;
                }
            }
        };
        let (mut stream, _) = accepted?;
        let hello = serde_json::json!({
            "codegraph": env!("CARGO_PKG_VERSION"),
            "pid": std::process::id(),
            "addr": addr,
            "protocol": 1,
        });
        stream.write_all(hello.to_string().as_bytes()).await?;
        stream.write_all(b"\n").await?;
        stream.flush().await?;

        let service = service.clone();
        let done_tx = done_tx.clone();
        active_connections += 1;
        tokio::spawn(async move {
            let (read, write) = stream.into_split();
            match rmcp::serve_server(service, (read, write)).await {
                Ok(running) => {
                    let _ = running.waiting().await;
                }
                Err(error) => {
                    log::debug!("daemon MCP session ended during initialization: {}", error);
                }
            }
            let _ = done_tx.send(());
        });
    }

    let _ = std::fs::remove_file(pid_path);
    Ok(())
}

fn daemon_idle_timeout() -> std::time::Duration {
    std::env::var("CODEGRAPH_DAEMON_IDLE_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(std::time::Duration::from_millis)
        .unwrap_or_else(|| std::time::Duration::from_millis(300_000))
}
