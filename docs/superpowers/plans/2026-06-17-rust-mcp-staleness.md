# Rust MCP Staleness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox
> (`- [ ]`) syntax for tracking.

**Goal:** Add stale-index warnings to Rust MCP tool responses without waiting on OS watcher events.

**Architecture:** Reuse indexed file hashes from SQLite. On each MCP tool call, compare indexed hashes with current
disk content for indexed files, then decorate successful text responses with a banner or footer. Keep status JSON
machine-readable by adding `pendingSync`.

**Tech Stack:** Rust stdlib file IO, existing `sha2`, existing `QueryService` and `mcp::tools`.

---

### Task 1: Add Failing MCP Staleness Tests

**Files:**
- Modify: `tests/cli_query_mcp_parity.rs`

- [x] **Step 1: Add referenced-file banner test**

Index a TypeScript fixture, edit the file without syncing, call `codegraph_search`, and assert the text starts with a
warning containing the edited path and the search result.

- [x] **Step 2: Add elsewhere footer test**

Edit a different indexed file, call `codegraph_search` for an unchanged symbol, and assert the response has an
elsewhere pending-sync footer.

- [x] **Step 3: Add status JSON test**

Allowlist `status`, edit an indexed file, call `codegraph_status`, and assert `pendingSync[0].path` is present.

- [x] **Step 4: Confirm RED**

Run:

```powershell
cargo test --test cli_query_mcp_parity mcp_staleness -- --nocapture --test-threads=1
```

Expected: tests fail because Rust MCP currently returns no stale warning metadata.

### Task 2: Implement Hash-Based Staleness Decoration

**Files:**
- Modify: `src/query_service.rs`
- Modify: `src/mcp/tools.rs`

- [x] **Step 1: Add pending-sync detector**

Add `QueryService::pending_sync_files()` that reads indexed files, hashes current disk content, and returns files whose
hash differs or file is missing.

- [x] **Step 2: Decorate successful text responses**

After each successful MCP tool call, prepend banner when the response text references stale files, otherwise append an
elsewhere footer when stale files exist.

- [x] **Step 3: Preserve status JSON**

Add `pendingSync` to the `codegraph_status` JSON instead of decorating status with banner/footer.

### Task 3: Verify and Commit

**Files:**
- Modify: `docs/superpowers/specs/2026-06-17-rust-mcp-staleness-design.md`
- Modify: `docs/superpowers/plans/2026-06-17-rust-mcp-staleness.md`
- Modify: `src/query_service.rs`
- Modify: `src/mcp/tools.rs`
- Modify: `tests/cli_query_mcp_parity.rs`

- [x] **Step 1: Run focused tests**

```powershell
cargo test --test cli_query_mcp_parity mcp_staleness -- --nocapture --test-threads=1
```

- [x] **Step 2: Run parity and daemon tests**

```powershell
cargo test --test cli_query_mcp_parity -- --nocapture --test-threads=1
cargo test --test mcp_daemon_parity -- --nocapture --test-threads=1
```

- [x] **Step 3: Run build and diff checks**

```powershell
cargo check
git diff --check
```

- [ ] **Step 4: Commit**

```powershell
git add docs/superpowers/specs/2026-06-17-rust-mcp-staleness-design.md `
    docs/superpowers/plans/2026-06-17-rust-mcp-staleness.md `
    src/query_service.rs src/mcp/tools.rs tests/cli_query_mcp_parity.rs
git commit -m "feat(mcp): 添加索引过期提示"
```
