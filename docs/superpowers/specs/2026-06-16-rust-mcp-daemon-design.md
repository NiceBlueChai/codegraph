<!--
  Rust MCP daemon design for migrating the TypeScript shared-daemon runtime.
  This document records the approved scope before implementation planning begins.
-->

# Rust MCP Daemon 设计

日期：2026-06-16

## 背景

当前 Rust MCP server 已能通过 stdio 暴露核心工具：`codegraph_explore`、`codegraph_node`、
`codegraph_search`、`codegraph_callers`，以及 allowlist 启用的 `callees`、`impact`、`files`、
`status`。这些工具走 Rust `QueryService`，可以服务单个 MCP 客户端。

TypeScript 版还有一层共享 daemon 架构：同一项目只启动一个后台 daemon，多个 MCP 客户端通过
proxy 连接它。这样可以复用项目状态、数据库连接、watcher 和 warmup 成本，并避免一个客户端退出就
带走所有其他会话。

本阶段目标是迁移共享 daemon 的必要运行语义，并把 MCP 协议处理切到官方 Rust SDK。daemon 只负责
进程复用和连接复用，不继续扩写自研 JSON-RPC 协议层。

## 目标

1. `codegraph serve --mcp` 默认尝试连接或启动项目级 daemon。
2. 同一已初始化项目同时启动多个 MCP 会话时，只产生一个 daemon。
3. 每个 launcher 作为 proxy，把 stdio JSON-RPC 行转发到 daemon socket。
4. `CODEGRAPH_NO_DAEMON` 为 truthy 时保留 direct stdio MCP 行为；空、`0`、`false` 不禁用 daemon。
5. daemon pidfile 可检测 stale daemon，并允许后来的 launcher 清理后接管。
6. daemon 空闲一段时间后退出并清理 pidfile。
7. daemon 版本与 launcher 版本不匹配时，launcher 不连接旧 daemon，回退 direct mode。
8. MCP tool surface 和 handler 继续复用现有 Rust MCP 实现。
9. MCP 协议、schema、stdio transport 优先使用官方 `rmcp` crate。

## 非目标

- 不迁移 TypeScript local-handshake proxy 的即时 `initialize` / `tools/list` 优化。
- 不实现 daemon 中途死亡时 proxy 原地切换到 in-process engine。
- 不迁移共享 watcher；Rust watcher 后续单独切片。
- 不实现 Windows named pipe / Unix domain socket 双实现。第一版使用 `127.0.0.1:0` loopback TCP，并交给
  `rmcp` 的 async read/write transport 处理 MCP 帧。
- 不修改 CLI query/call graph 行为。

## 设计

### 依赖选择

使用官方 `rmcp` crate 作为 MCP 协议层。当前公开文档显示 `rmcp` 1.7.0 是官方 Rust MCP SDK，
提供 server 能力、工具系统、stdio transport，以及 generic async read/write transport。项目已使用
`tokio`，所以 daemon 的 TCP listener 和 per-session transport 也使用 `tokio`，不再新增 JSON-RPC crate。

不引入独立 daemon 管理库。pidfile、`create_new` 启动锁和 loopback TCP 足够覆盖本阶段需求。

### 运行模式

`serve --mcp` 分两种模式：

- direct mode：现有 stdio MCP server。触发条件是 `CODEGRAPH_NO_DAEMON` 为 truthy、项目未初始化，或 daemon
  连接/启动过程明确失败。
- daemon mode：默认路径。launcher 尝试连接 `.codegraph/daemon.pid` 指向的 daemon；连接失败则抢锁
  启动新 daemon；成功后作为 proxy 转发 stdio。

未初始化项目不创建 daemon，保持现有 inactive MCP 行为。

### pidfile

pidfile 路径：`.codegraph/daemon.pid`。

内容：

```json
{
  "pid": 12345,
  "version": "0.1.0",
  "addr": "127.0.0.1:49152",
  "startedAt": 1781580000000
}
```

写入规则：

1. launcher 先创建 `.codegraph/daemon.starting.lock`，用 `create_new(true)` 做进程间互斥。
2. 持锁期间检查 pidfile；若 pid 存活且版本匹配且 addr 可连接，释放锁并走 proxy。
3. 若 pid 不存活或连接失败，删除 pidfile。
4. spawn detached daemon。
5. daemon 绑定 `127.0.0.1:0` 后写 pidfile。
6. launcher 轮询 pidfile 并连接 daemon。

### daemon

daemon 是独立进程：

```text
codegraph serve --mcp-daemon --path <project-root>
```

daemon 启动后：

1. 打开项目和数据库。
2. 监听 loopback TCP。
3. 每个连接用 `rmcp` async read/write transport 启动一个 MCP session。
4. session 使用同一 project root；数据库连接可以按 session 打开，先不共享 rusqlite connection。
5. 连接数归零后启动 idle timer；超时退出。
6. 收到 SIGINT/SIGTERM 或 idle timeout 时删除 pidfile。

默认 idle timeout：`CODEGRAPH_DAEMON_IDLE_TIMEOUT_MS`，未设置时 300000 ms。

### proxy

proxy 运行在 launcher 进程里：

1. 连接 daemon addr。
2. 读取 daemon hello 行：

```json
{"codegraph":"0.1.0","pid":12345,"addr":"127.0.0.1:49152","protocol":1}
```

3. 版本匹配后开始双向转发：
   - stdin line -> daemon socket
   - daemon socket line -> stdout
4. 任一端关闭时退出。

第一版 proxy 不解析 JSON-RPC 内容，不做请求级 fallback。

AI 客户端配置仍然使用 stdio：

```json
{
  "mcpServers": {
    "codegraph": {
      "type": "stdio",
      "command": "codegraph",
      "args": ["serve", "--mcp", "--path", "<project-root>"]
    }
  }
}
```

`serve --mcp` 是客户端看到的 stdio 进程；daemon socket/TCP 是 CodeGraph 内部连接，不写进 AI 客户端配置。

### 错误处理

- pidfile 无法解析：视为 stale，删除并重试启动。
- pid 存活但版本不匹配：不删除 pidfile，direct mode fallback。
- pid 存活但 addr 连接失败：若 pid 不存活则清理；若 pid 存活则 direct mode fallback，避免误杀。
- daemon 启动超时：direct mode fallback。
- proxy 连接中断：launcher 退出；客户端下次启动会重新连接或启动 daemon。

### 测试策略

新增 Rust 集成测试 `tests/mcp_daemon_parity.rs`，使用真实 `codegraph` 二进制和临时已初始化项目：

1. 两个 `serve --mcp` 会话共享一个 daemon。
2. 三个并发 launcher 只创建一个 daemon。
3. `CODEGRAPH_NO_DAEMON=true` 不创建 pidfile；`CODEGRAPH_NO_DAEMON=false` 仍启用 daemon。
4. stale pidfile 被清理，新 daemon 接管。
5. 版本不匹配时 direct mode 仍能响应 initialize。
6. 关闭最后一个 client 后 daemon idle timeout 并删除 pidfile。
7. 通过 daemon path 调用 `tools/list` 和 `codegraph_status`。

保留现有 `tests/cli_query_mcp_parity.rs` MCP 测试，确保 tool surface 不回退。

## 成功标准

- `cargo test --test mcp_daemon_parity` 通过。
- `cargo test --test cli_query_mcp_parity mcp` 通过。
- `cargo check` 通过。
- 手动启动两个 `codegraph serve --mcp --path <project>` 时，`.codegraph/daemon.pid` 中 pid 相同。

## 参考

- `rmcp` 是官方 Rust MCP SDK，提供 server、tool system、stdio transport 和 generic async read/write
  transport。
- https://docs.rs/rmcp/latest/rmcp/
- https://github.com/modelcontextprotocol/rust-sdk
