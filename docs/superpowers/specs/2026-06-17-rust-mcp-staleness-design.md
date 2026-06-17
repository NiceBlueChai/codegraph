<!--
  Rust MCP staleness banner design for migrating the useful TypeScript stale-index behavior.
-->

# Rust MCP Staleness 设计

日期：2026-06-17

## 目标

迁移 MCP 响应里的 stale-index 提示能力。Rust 版先不依赖 OS watcher 事件；每次 MCP tool 调用时，
用数据库里的 indexed file `content_hash` 对比磁盘当前文件内容，得到当前仍未被 `codegraph sync` 吸收的文件。

## 范围

- `codegraph_search`、`codegraph_node`、`codegraph_explore` 等文本响应根据引用文件加 banner 或 footer。
- `codegraph_status` 保持 JSON，可额外返回 `pendingSync` 数组。
- `codegraph sync` 后 hash 更新，提示自然消失。

## 行为

- 如果响应文本提到某个 stale 文件，前置 warning banner，提示直接读取文件。
- 如果项目有 stale 文件但当前响应没有引用这些文件，追加 footer。
- `codegraph_status` 返回 `pendingSync: [{ path, ageMs }]`。

## 非目标

- 本切片不接入 native watcher。
- 本切片不做自动 sync。
- 本切片不实现 `upgrade`。
