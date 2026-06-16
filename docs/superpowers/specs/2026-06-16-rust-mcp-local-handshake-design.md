<!--
  Rust MCP local-handshake design for matching the TypeScript daemon launcher behavior.
-->

# Rust MCP Local Handshake 设计

日期：2026-06-16

## 目标

迁移 TypeScript 版 `serve --mcp` 的 local-handshake 行为：launcher 进程先本地响应
`initialize`、`tools/list`、`resources/list`、`resources/templates/list` 和 `prompts/list`，
避免 daemon 冷启动或启动锁等待导致 MCP 客户端在首轮看不到工具。

## 范围

- 保留现有 Rust daemon、pidfile、启动锁和 direct fallback。
- 只修改 launcher proxy 路径，不重写 `rmcp` service。
- 工具调用仍优先转发给 shared daemon。
- 如果 daemon 连接失败，launcher 用当前项目的 Rust MCP tool handler 本进程兜底处理 `tools/call`
  和 `ping`，并对不能处理的请求返回 JSON-RPC 错误。

## 行为

`serve --mcp` 在已初始化项目且未设置 `CODEGRAPH_NO_DAEMON` 时进入 local-handshake proxy：

1. 后台线程执行现有 `connect_or_spawn`。
2. 主线程立刻读取 stdin JSON-RPC 行。
3. 本地响应：
   - `initialize`：返回 CodeGraph server info、tools capability、instructions。
   - `tools/list`：返回当前 `CODEGRAPH_MCP_TOOLS` 下的静态工具列表。
   - `resources/list`：返回空 resources。
   - `resources/templates/list`：返回空 resourceTemplates。
   - `prompts/list`：返回空 prompts。
4. 其他消息在 daemon ready 前排队。
5. daemon ready 后，排队消息写入 daemon socket；转发 daemon 输出到 stdout。
6. 转发过的 `initialize` 响应会被 suppress，因为客户端已经收到本地响应。
7. daemon 失败时，排队的 `tools/call` 在本进程执行；其他带 id 的请求返回错误。

## 非目标

- 本切片不实现 `upgrade`。
- 本切片不迁移 watcher staleness banner。
- 本切片不迁移 daemon 中途死亡后的完整 in-flight replay；后续切片再做。

## 验收

- daemon 启动锁被占用时，`initialize` 和 `tools/list` 仍能快速返回。
- `resources/list` 和 `prompts/list` 返回空列表而不是方法不存在。
- daemon 正常启动后，`tools/call` 仍通过 shared daemon 返回结果，并产生 pidfile。
- `CODEGRAPH_NO_DAEMON=true` 保持 direct stdio 行为。
