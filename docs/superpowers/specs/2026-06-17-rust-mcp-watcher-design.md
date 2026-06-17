<!--
  Rust MCP watcher design for enabling automatic sync in the shared daemon.
-->

# Rust MCP Watcher 设计

日期：2026-06-17

## 目标

让 Rust MCP shared daemon 启动项目级 watcher。文件变化后 watcher 通过现有
`Indexer::sync_files` 更新 SQLite 索引，使 MCP 查询逐步接近 TypeScript 版的自动刷新行为。

## 范围

- daemon 默认启动 watcher。
- `serve --mcp --no-watch` 传递给隐藏 daemon，并关闭 watcher。
- watcher 只调用现有 `sync_files`，不新建索引流程。
- 继续保留上一切片的 hash-based staleness 提示，作为 watcher 事件延迟或禁用时的兜底。

## 非目标

- 本切片不重写 watcher policy。
- 本切片不迁移 TS watcher 的测试专用 synthetic event seam。
- 本切片不实现 `upgrade`。
