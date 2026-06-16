# Rust MCP Local Handshake Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox
> (`- [ ]`) syntax for tracking.

**Goal:** Make Rust `serve --mcp` expose MCP tools immediately while the shared daemon connects in the background.

**Architecture:** Keep the existing daemon and `rmcp` service. Replace the launcher-side transparent proxy with a
small line-oriented JSON-RPC proxy that answers static handshake/list requests locally, buffers tool calls until the
daemon socket is ready, and falls back to the local Rust tool handler if daemon startup fails.

**Tech Stack:** Rust stdlib threads/channels/TCP, `serde_json`, existing `mcp::tools`, existing `ProjectContext`.

---

### Task 1: Add Local-Handshake Regression Tests

**Files:**
- Modify: `tests/mcp_daemon_parity.rs`

- [x] **Step 1: Add helpers for multi-message stdio sessions**

Add a helper that spawns `codegraph serve --mcp --path <project>`, writes JSON-RPC lines, and reads stdout responses
by id with a caller-supplied timeout.

- [x] **Step 2: Add locked-daemon startup test**

Create `.codegraph/daemon.starting.lock`, send `initialize` and `tools/list`, and assert both responses arrive within
one second. This fails before implementation because the current proxy waits up to five seconds for daemon startup.

- [x] **Step 3: Add local empty-list probes**

Send `resources/list`, `resources/templates/list`, and `prompts/list`; assert empty list results.

- [x] **Step 4: Run tests and confirm RED**

Run:

```powershell
cargo test --test mcp_daemon_parity local_handshake -- --nocapture --test-threads=1
```

Expected: at least the locked-daemon startup test fails or times out before implementation.

### Task 2: Implement Local Handshake Proxy

**Files:**
- Modify: `src/mcp/proxy.rs`
- Modify: `src/mcp/tools.rs`
- Modify: `src/mcp/daemon.rs`

- [x] **Step 1: Expose static MCP tool JSON**

Add a small public helper in `src/mcp/tools.rs` that returns registered tools as JSON objects for manual
`tools/list` responses.

- [x] **Step 2: Start daemon connection in the background**

Change `proxy::run` so it spawns `connect_or_spawn(project)` on a thread and immediately enters the local proxy loop.

- [x] **Step 3: Answer static requests locally**

In the proxy loop, parse newline-delimited JSON-RPC. Locally answer `initialize`, `tools/list`, `resources/list`,
`resources/templates/list`, and `prompts/list`.

- [x] **Step 4: Forward or locally handle remaining requests**

Buffer non-static messages until the daemon connection resolves. If daemon succeeds, forward buffered and future
messages to the daemon. If daemon fails, handle `tools/call` via `tools::execute_tool`, answer `ping`, and return a
JSON-RPC error for unsupported request methods.

- [x] **Step 5: Suppress forwarded initialize response**

Record the initialize id answered locally and avoid writing the daemon response with the same id to stdout.

### Task 3: Verify and Commit

**Files:**
- Modify: `docs/superpowers/specs/2026-06-16-rust-mcp-local-handshake-design.md`
- Modify: `docs/superpowers/plans/2026-06-16-rust-mcp-local-handshake.md`
- Modify: `src/mcp/daemon.rs`
- Modify: `src/mcp/proxy.rs`
- Modify: `src/mcp/tools.rs`
- Modify: `tests/mcp_daemon_parity.rs`

- [x] **Step 1: Run focused tests**

```powershell
cargo test --test mcp_daemon_parity -- --nocapture --test-threads=1
```

- [x] **Step 2: Run build check**

```powershell
cargo check
```

- [x] **Step 3: Check whitespace**

```powershell
git diff --check
```

- [ ] **Step 4: Commit**

```powershell
git add docs/superpowers/specs/2026-06-16-rust-mcp-local-handshake-design.md `
    docs/superpowers/plans/2026-06-16-rust-mcp-local-handshake.md `
    src/mcp/daemon.rs src/mcp/proxy.rs src/mcp/tools.rs tests/mcp_daemon_parity.rs
git commit -m "feat(mcp): 迁移本地握手代理"
```
