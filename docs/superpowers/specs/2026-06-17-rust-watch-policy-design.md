<!--
  Rust watch policy design for matching TypeScript watcher enable/disable behavior.
-->

# Rust Watch Policy 设计

日期：2026-06-17

## 目标

迁移 TypeScript 版 watcher policy：`CODEGRAPH_NO_WATCH` 显式关闭 watcher，`CODEGRAPH_FORCE_WATCH`
覆盖自动禁用，WSL `/mnt/<drive>` 路径默认禁用 watcher。

## 范围

- `FileWatcher::start` 在 policy 禁用时直接返回成功，但保持 inactive。
- daemon watcher 复用同一判断。
- 保留 `--no-watch` 传参；env policy 是额外入口。

## 非目标

- 不改 installer 提示。
- 不迁移全部 TS watcher 测试 seam。
- 不实现 `upgrade`。
