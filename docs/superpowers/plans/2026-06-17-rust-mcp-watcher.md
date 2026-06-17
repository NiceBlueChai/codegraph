# Rust MCP Watcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox
> (`- [ ]`) syntax for tracking.

**Goal:** Start Rust file watching from the shared MCP daemon and auto-sync changed files.

**Architecture:** Keep the existing `FileWatcher` and `Indexer::sync_files`. Add a watcher startup helper in the
daemon path, keep the watcher alive for the daemon lifetime, and pass `--no-watch` from launcher to hidden daemon.

**Tech Stack:** Rust stdlib, `notify`, existing `FileWatcher`, existing `Indexer`.

---

### Task 1: Add Failing Daemon Watcher Test

**Files:**
- Modify: `tests/mcp_daemon_parity.rs`

- [x] **Step 1: Add daemon spawn helper with env and args**

Extend the existing daemon test helper so a test can pass `CODEGRAPH_WATCH_DEBOUNCE_MS=100`.

- [x] **Step 2: Add auto-sync test**

Start `serve --mcp-daemon`, edit `app.ts`, wait until `codegraph node runApp --path <project>` shows the edited
source, and fail if it does not update within a few seconds.

- [x] **Step 3: Confirm RED**

Run:

```powershell
cargo test --test mcp_daemon_parity daemon_watcher -- --nocapture --test-threads=1
```

Expected: test fails because daemon currently never starts `FileWatcher`.

### Task 2: Start Watcher in Daemon

**Files:**
- Modify: `src/mcp/daemon.rs`
- Modify: `src/mcp/proxy.rs`
- Modify: `src/main.rs`
- Modify: `src/sync/watcher.rs`

- [x] **Step 1: Accept watch flag in daemon runner**

Change daemon entrypoint to `run_daemon(project, watch_enabled)`.

- [x] **Step 2: Start watcher when enabled**

Create a `FileWatcher`, start it with a callback that opens the project DB and calls `Indexer::sync_files(files)`,
and keep it in scope until daemon exits.

- [x] **Step 3: Pass `--no-watch` through launcher**

Change `cmd_serve` and `proxy::run` so `serve --mcp --no-watch` spawns hidden daemon with `--no-watch`.

### Task 3: Verify and Commit

**Files:**
- Modify: `docs/superpowers/specs/2026-06-17-rust-mcp-watcher-design.md`
- Modify: `docs/superpowers/plans/2026-06-17-rust-mcp-watcher.md`
- Modify: `src/mcp/daemon.rs`
- Modify: `src/mcp/proxy.rs`
- Modify: `src/main.rs`
- Modify: `src/sync/watcher.rs`
- Modify: `tests/mcp_daemon_parity.rs`

- [x] **Step 1: Run focused watcher test**

```powershell
cargo test --test mcp_daemon_parity daemon_watcher -- --nocapture --test-threads=1
```

- [x] **Step 2: Run parity tests**

```powershell
cargo test --test mcp_daemon_parity -- --nocapture --test-threads=1
cargo test --test cli_query_mcp_parity -- --nocapture --test-threads=1
```

- [x] **Step 3: Run build and diff checks**

```powershell
cargo check
git diff --check
```

- [ ] **Step 4: Commit**

```powershell
git add docs/superpowers/specs/2026-06-17-rust-mcp-watcher-design.md `
    docs/superpowers/plans/2026-06-17-rust-mcp-watcher.md `
    src/mcp/daemon.rs src/mcp/proxy.rs src/main.rs src/sync/watcher.rs tests/mcp_daemon_parity.rs
git commit -m "feat(mcp): 启用daemon文件监听同步"
```
