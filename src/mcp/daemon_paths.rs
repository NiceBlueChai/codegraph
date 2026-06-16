//! Daemon lock and pidfile helpers for project-scoped MCP daemon reuse.
//!
//! The daemon launcher uses these helpers to coordinate one background process
//! per initialized CodeGraph project without pulling in a process-management
//! dependency.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// JSON payload persisted in `.codegraph/daemon.pid`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonLockInfo {
    /// Operating-system process id for the daemon.
    pub pid: u32,
    /// CodeGraph package version that wrote the pidfile.
    pub version: String,
    /// Loopback TCP address the daemon listens on.
    pub addr: String,
    /// Unix timestamp in milliseconds when the daemon started.
    pub started_at: i64,
}

/// Returns the project-scoped daemon pidfile path.
pub fn daemon_pid_path(project_root: impl AsRef<Path>) -> PathBuf {
    codegraph_dir(project_root).join("daemon.pid")
}

/// Returns the project-scoped daemon startup lock path.
pub fn daemon_starting_lock_path(project_root: impl AsRef<Path>) -> PathBuf {
    codegraph_dir(project_root).join("daemon.starting.lock")
}

/// Reads daemon lock info from a JSON pidfile.
pub fn read_daemon_lock(path: impl AsRef<Path>) -> anyhow::Result<DaemonLockInfo> {
    let text = fs::read_to_string(path.as_ref())?;
    Ok(serde_json::from_str(&text)?)
}

/// Writes daemon lock info as JSON, creating the `.codegraph` directory if needed.
pub fn write_daemon_lock(path: impl AsRef<Path>, info: &DaemonLockInfo) -> anyhow::Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path.as_ref(), serde_json::to_string_pretty(info)? + "\n")?;
    Ok(())
}

/// Checks whether an OS process id appears to be alive.
pub fn pid_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    platform_pid_is_alive(pid)
}

fn codegraph_dir(project_root: impl AsRef<Path>) -> PathBuf {
    match std::env::var_os("CODEGRAPH_DIR") {
        Some(dir) => project_root.as_ref().join(dir),
        None => project_root.as_ref().join(".codegraph"),
    }
}

#[cfg(windows)]
fn platform_pid_is_alive(pid: u32) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|stdout| stdout.lines().any(|line| line.contains(&pid.to_string())))
}

#[cfg(unix)]
fn platform_pid_is_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(not(any(windows, unix)))]
fn platform_pid_is_alive(_pid: u32) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_paths_live_under_codegraph_dir() {
        let root = Path::new("project");

        assert_eq!(
            daemon_pid_path(root),
            PathBuf::from("project")
                .join(".codegraph")
                .join("daemon.pid")
        );
        assert_eq!(
            daemon_starting_lock_path(root),
            PathBuf::from("project")
                .join(".codegraph")
                .join("daemon.starting.lock")
        );
    }

    #[test]
    fn daemon_lock_round_trips_json() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = daemon_pid_path(dir.path());
        let info = DaemonLockInfo {
            pid: 123,
            version: "0.1.0".to_string(),
            addr: "127.0.0.1:49152".to_string(),
            started_at: 1781580000000,
        };

        write_daemon_lock(&path, &info).expect("write lock");

        assert_eq!(read_daemon_lock(&path).expect("read lock"), info);
    }

    #[test]
    fn current_process_pid_is_alive() {
        assert!(pid_is_alive(std::process::id()));
        assert!(!pid_is_alive(0));
    }
}
