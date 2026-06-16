//! Project-scoped MCP daemon listener.
//!
//! The daemon owns the long-lived TCP listener. Each accepted connection is
//! handed to the same rmcp-backed service used by direct stdio mode.

use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

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
    let info = DaemonLockInfo {
        pid: std::process::id(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        addr: addr.clone(),
        started_at: chrono::Utc::now().timestamp_millis(),
    };
    write_daemon_lock(daemon_pid_path(&project.root), &info)?;
    let service = CodeGraphMcpService::active(project);

    loop {
        let (mut stream, _) = listener.accept().await?;
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
        });
    }
}
