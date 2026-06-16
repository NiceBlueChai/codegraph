<!--
  Implementation plan for the first Rust MCP shared-daemon slice.
  Uses the official rmcp crate for protocol handling.
-->

# Rust MCP Daemon Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Rust `codegraph serve --mcp` use the official Rust MCP SDK and share one project-level daemon across MCP clients.

**Architecture:** First replace the hand-written Rust MCP protocol dispatcher with a thin `rmcp` service adapter around existing `QueryService` handlers. Then add a small daemon/proxy layer that reuses the same `rmcp` service over loopback TCP.

**Tech Stack:** Rust 2021, `rmcp`, tokio, std::process, serde/serde_json, existing CodeGraph query/index modules, Cargo integration tests.

---

## File Structure

- Modify `Cargo.toml`: add `rmcp` with server, stdio, async read/write, and schema features.
- Create `src/mcp/service.rs`: `rmcp` service adapter for CodeGraph tools.
- Keep `src/mcp/tools.rs`: tool definitions and service-backed handler functions; convert them to the shape `rmcp` expects.
- Modify `src/mcp/server.rs`: replace custom JSON-RPC loop with `rmcp` stdio/async-read-write serving.
- Create `src/mcp/daemon_paths.rs`: daemon pidfile paths, lockfile paths, lock info JSON, pid liveness helpers.
- Create `src/mcp/daemon.rs`: tokio TCP daemon listener, per-client `rmcp` session, idle shutdown.
- Create `src/mcp/proxy.rs`: launcher-side connect/spawn/proxy logic.
- Modify `src/mcp/mod.rs`: export new modules.
- Modify `src/main.rs`: add hidden daemon command and route `serve --mcp` through proxy unless disabled.
- Create `tests/mcp_daemon_parity.rs`: process-level daemon/proxy integration tests.
- Keep `tests/cli_query_mcp_parity.rs`: existing MCP surface tests.

## Task 1: Add rmcp dependency and preserve current direct MCP behavior

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `src/mcp/server.rs`
- Create: `src/mcp/service.rs`
- Modify: `src/mcp/mod.rs`
- Test: `tests/cli_query_mcp_parity.rs`

- [ ] **Step 1: Add dependency**

Add to `Cargo.toml`. The version and features come from the current `rmcp` 1.7.0 docs:

```toml
rmcp = { version = "1.7", features = ["server", "macros", "schemars", "transport-io", "transport-async-rw"] }
```

Run: `cargo update -p rmcp`

- [ ] **Step 2: Write a failing compatibility check**

Run existing MCP tests before changing protocol code:

```bash
cargo test --test cli_query_mcp_parity mcp -- --nocapture
```

Expected before adapter work: current custom server passes. Keep this as the compatibility baseline.

- [ ] **Step 3: Create rmcp service adapter**

Create `src/mcp/service.rs` with a small service struct:

```rust
//! rmcp service adapter for CodeGraph MCP tools.

use crate::db::QueryBuilder;
use crate::mcp::tools;
use crate::project::Project;
use rmcp::handler::server::ServerHandler;

#[derive(Clone)]
pub struct CodeGraphMcpService {
    project: Option<Project>,
}

impl CodeGraphMcpService {
    pub fn active(project: Project) -> Self {
        Self { project: Some(project) }
    }

    pub fn inactive() -> Self {
        Self { project: None }
    }
}

impl ServerHandler for CodeGraphMcpService {}
```

Then adapt `tools/list` and `tools/call` through `rmcp` APIs using the existing `tools::register_tools()` and
`tools::execute_tool(...)` functions. Keep the implementation thin; do not duplicate tool behavior.

- [ ] **Step 4: Replace custom direct server loop**

In `src/mcp/server.rs`, keep public construction shape but delegate stdio serving to `rmcp`:

```rust
pub fn run_stdio(project: Option<Project>) -> anyhow::Result<()> {
    let service = match project {
        Some(project) => CodeGraphMcpService::active(project),
        None => CodeGraphMcpService::inactive(),
    };
    // Use rmcp transport-io stdio server here.
    // Keep all CodeGraph behavior inside CodeGraphMcpService/tools.rs.
    Ok(())
}
```

Use the `rmcp::serve_server` path with `rmcp::transport::stdio()` for direct stdio serving. Do not keep
hand-written JSON-RPC parsing once `rmcp` serves stdio.

- [ ] **Step 5: Verify direct MCP parity**

Run:

```bash
cargo test --test cli_query_mcp_parity mcp -- --nocapture
cargo check
```

Expected: all MCP parity tests still pass.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/mcp/service.rs src/mcp/server.rs src/mcp/mod.rs src/mcp/tools.rs
git commit -m "refactor(mcp): 使用rmcp协议库"
```

## Task 2: Add daemon pidfile helpers

**Files:**
- Create: `src/mcp/daemon_paths.rs`
- Modify: `src/mcp/mod.rs`

- [ ] **Step 1: Add pidfile helper tests and implementation**

Create `src/mcp/daemon_paths.rs` with:

- `DaemonLockInfo { pid, version, addr, started_at }`
- `daemon_pid_path(project_root)`
- `daemon_starting_lock_path(project_root)`
- `read_daemon_lock(path)`
- `write_daemon_lock(path, info)`
- `pid_is_alive(pid)`

Use `serde_json` and stdlib only. Use `OpenOptions::create_new(true)` later for the starting lock.

- [ ] **Step 2: Run helper tests**

Run:

```bash
cargo test mcp::daemon_paths
```

Expected: helper unit tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/mcp/daemon_paths.rs src/mcp/mod.rs
git commit -m "feat(mcp): 添加daemon锁文件辅助"
```

## Task 3: Add hidden daemon process mode

**Files:**
- Create: `src/mcp/daemon.rs`
- Modify: `src/mcp/mod.rs`
- Modify: `src/main.rs`
- Test: `tests/mcp_daemon_parity.rs`

- [ ] **Step 1: Add failing daemon initialize test**

Create `tests/mcp_daemon_parity.rs` with helpers that:

- create a temp project with `codegraph init`
- spawn `codegraph serve --mcp-daemon --path <temp>`
- send an MCP `initialize` request
- assert the response has `serverInfo.name == "codegraph"`

Expected before implementation: clap rejects `--mcp-daemon`.

- [ ] **Step 2: Implement daemon listener**

Create `src/mcp/daemon.rs`:

- bind `tokio::net::TcpListener` to `127.0.0.1:0`
- write `.codegraph/daemon.pid`
- send a one-line daemon hello to each client before MCP traffic:

```json
{"codegraph":"0.1.0","pid":12345,"addr":"127.0.0.1:49152","protocol":1}
```

- split accepted `TcpStream` and serve `CodeGraphMcpService` using `rmcp` async read/write transport

- [ ] **Step 3: Add hidden command**

In `src/main.rs`, add hidden command:

```rust
#[command(hide = true)]
McpDaemon {
    #[arg(long, default_value = ".")]
    path: String,
}
```

Route it to `mcp::daemon::run_daemon`.

- [ ] **Step 4: Verify daemon direct test**

Run:

```bash
cargo test --test mcp_daemon_parity hidden_daemon_mode_answers_initialize -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/mcp/daemon.rs src/mcp/mod.rs src/main.rs tests/mcp_daemon_parity.rs
git commit -m "feat(mcp): 添加rust daemon服务模式"
```

## Task 4: Add launcher proxy and default daemon path

**Files:**
- Create: `src/mcp/proxy.rs`
- Modify: `src/mcp/mod.rs`
- Modify: `src/main.rs`
- Test: `tests/mcp_daemon_parity.rs`

- [ ] **Step 1: Add failing shared-daemon test**

Add test:

- spawn first `codegraph serve --mcp`
- send initialize
- read `.codegraph/daemon.pid`
- spawn second `codegraph serve --mcp`
- send initialize
- assert pidfile pid is unchanged

Expected before implementation: no shared daemon behavior.

- [ ] **Step 2: Implement proxy**

Create `src/mcp/proxy.rs`:

- read pidfile
- if version matches and addr connects, consume daemon hello and proxy stdin/stdout bytes
- if missing/stale, acquire `.codegraph/daemon.starting.lock` with `create_new(true)`
- spawn detached `codegraph serve --mcp-daemon --path <root>`
- wait up to 5 seconds for pidfile/addr
- proxy bytes without parsing JSON-RPC

- [ ] **Step 3: Route `serve --mcp`**

In `cmd_serve`, if `CODEGRAPH_NO_DAEMON != 1` and project resolves, run proxy. If proxy setup fails, fall back to direct `rmcp` stdio service.

- [ ] **Step 4: Verify shared daemon**

Run:

```bash
cargo test --test mcp_daemon_parity two_mcp_launchers_share_one_daemon -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/mcp/proxy.rs src/mcp/mod.rs src/main.rs tests/mcp_daemon_parity.rs
git commit -m "feat(mcp): 通过proxy复用daemon"
```

## Task 5: Cover opt-out, stale locks, version mismatch, and idle shutdown

**Files:**
- Modify: `src/mcp/daemon.rs`
- Modify: `src/mcp/proxy.rs`
- Modify: `tests/mcp_daemon_parity.rs`

- [ ] **Step 1: Add tests**

Add tests for:

- `CODEGRAPH_NO_DAEMON=1` responds to initialize and creates no pidfile
- stale pidfile with dead pid is replaced
- pidfile version mismatch falls back to direct mode
- daemon exits after last client disconnects and `CODEGRAPH_DAEMON_IDLE_TIMEOUT_MS` elapses

- [ ] **Step 2: Implement missing behavior**

Keep it minimal:

- stale dead pid -> remove pidfile
- live mismatched pid -> direct fallback
- no daemon env -> direct mode
- idle shutdown -> remove pidfile before exit

- [ ] **Step 3: Verify task tests**

Run:

```bash
cargo test --test mcp_daemon_parity -- --nocapture
```

Expected: all daemon tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/mcp/daemon.rs src/mcp/proxy.rs tests/mcp_daemon_parity.rs
git commit -m "test(mcp): 覆盖daemon生命周期"
```

## Task 6: Full verification

**Files:**
- Modify only if verification finds a defect.

- [ ] **Step 1: Run daemon tests**

Run:

```bash
cargo test --test mcp_daemon_parity -- --nocapture
```

Expected: all tests pass.

- [ ] **Step 2: Run existing MCP parity tests**

Run:

```bash
cargo test --test cli_query_mcp_parity mcp -- --nocapture
```

Expected: all filtered MCP tests pass.

- [ ] **Step 3: Run compiler check**

Run:

```bash
cargo check
```

Expected: exit 0.

- [ ] **Step 4: Run whitespace check**

Run:

```bash
git diff --check
```

Expected: no output.

- [ ] **Step 5: Commit final fixes only if needed**

```bash
git add <changed-files>
git commit -m "fix(mcp): 完善daemon验证"
```

## Self-Review

- Spec coverage: tasks cover official MCP SDK adoption, daemon default mode, direct opt-out, shared daemon, stale pidfile, version mismatch, idle cleanup, and existing MCP handlers.
- Placeholder scan: no unfinished placeholder markers remain.
- Scope check: local-handshake proxy, mid-session daemon fallback, named pipe, and watcher sharing are intentionally excluded and documented in the spec.
