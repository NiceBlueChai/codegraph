# Rust Watch Policy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox
> (`- [ ]`) syntax for tracking.

**Goal:** Make Rust file watching honor the TypeScript watch-disable policy.

**Architecture:** Keep policy logic inside `src/sync/watcher.rs` to avoid a new module. Add a small pure helper for
unit tests and call it at the top of `FileWatcher::start`.

**Tech Stack:** Rust stdlib env/path checks, existing `FileWatcher`.

---

### Task 1: Add Watch Policy Tests

**Files:**
- Modify: `src/sync/watcher.rs`

- [x] **Step 1: Add pure policy tests**

Cover `CODEGRAPH_NO_WATCH`, `CODEGRAPH_FORCE_WATCH`, WSL `/mnt/d`, WSL `/mnt/wsl`, and non-WSL `/mnt/d`.

- [x] **Step 2: Add start behavior test**

Set `CODEGRAPH_NO_WATCH=1`, start a watcher, and assert `is_running()` stays false.

- [x] **Step 3: Confirm RED**

Run:

```powershell
cargo test --lib watch_policy -- --nocapture
```

Expected: tests fail because Rust watcher currently ignores the policy.

### Task 2: Implement Policy

**Files:**
- Modify: `src/sync/watcher.rs`

- [x] **Step 1: Add policy helpers**

Add `watch_disabled_reason`, `watch_disabled_reason_with`, `detect_wsl`, and `is_windows_drive_mount`.

- [x] **Step 2: Gate `FileWatcher::start`**

Return `Ok(())` without starting native watchers when policy says disabled.

### Task 3: Verify and Commit

**Files:**
- Modify: `docs/superpowers/specs/2026-06-17-rust-watch-policy-design.md`
- Modify: `docs/superpowers/plans/2026-06-17-rust-watch-policy.md`
- Modify: `src/sync/watcher.rs`

- [x] **Step 1: Run tests**

```powershell
cargo test --lib watch_policy -- --nocapture
cargo test --lib -- --nocapture
cargo test --test mcp_daemon_parity -- --nocapture --test-threads=1
```

- [x] **Step 2: Run build and diff checks**

```powershell
cargo check
git diff --check
```

- [ ] **Step 3: Commit**

```powershell
git add docs/superpowers/specs/2026-06-17-rust-watch-policy-design.md `
    docs/superpowers/plans/2026-06-17-rust-watch-policy.md src/sync/watcher.rs
git commit -m "feat(watch): 对齐文件监听禁用策略"
```
