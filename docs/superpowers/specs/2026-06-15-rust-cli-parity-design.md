<!--
  Rust CLI parity design for migrating CodeGraph command behavior from the TypeScript main branch.
  This document records the approved scope before implementation planning begins.
-->

# Rust CLI 功能对齐设计

日期：2026-06-15

## 背景

当前 `rust-migration` 分支已经新增 Rust 实现，并补了一批 CLI 子命令，但它和 `main` 分支的 TypeScript 版本仍不是同一套外部行为。`MIGRATION_PROGRESS.md` 宣称迁移完成，但实际差异包括 MCP 工具面、`explore`/`node` 语义、安装器行为、未初始化处理、输出格式、路径兼容性、watcher staleness 行为，以及语言和框架覆盖深度。

目标不是让 Rust 版“看起来有类似命令”，而是让 Rust 重写版本覆盖原 `main` 分支的用户可见功能。TypeScript `main:src/bin/codegraph.ts`、`main:src/mcp/tools.ts`、`main:src/index.ts` 和现有测试作为行为基准。

## 已确认方案

采用“命令级垂直切片”的方案：

1. 逐条子命令按 TypeScript 行为重实现。
2. 每条命令先对齐参数、默认值、输出、JSON 结构、错误语义和退出行为。
3. CLI 与 MCP 共用 Rust 服务层，避免同一功能在两个入口各实现一套。
4. 语言和框架解析能力按后续垂直切片迁移，每个切片带覆盖测试。

首轮不实现 `upgrade`。`upgrade` 保留为后续阶段；首轮只要求它不误导用户，不执行不完整升级流程，并提供明确的未支持提示或从帮助中移出。

## 首轮范围

首轮覆盖以下原 `main` 子命令：

- `init`
- `uninit`
- `index`
- `sync`
- `status`
- `query`
- `explore`
- `node`
- `files`
- `serve`
- `unlock`
- `callers`
- `callees`
- `impact`
- `affected`
- `install`
- `uninstall`

`upgrade` 不在首轮实现范围。

首轮 MCP 覆盖：

- 默认工具面按 TypeScript 版对齐：`codegraph_explore`、`codegraph_node`、`codegraph_search`、`codegraph_callers`。
- `codegraph_callees`、`codegraph_impact`、`codegraph_files`、`codegraph_status` 保持可通过 `CODEGRAPH_MCP_TOOLS` 启用。
- 未初始化 workspace 时，MCP server 明确声明 inactive，并且 `tools/list` 返回空工具。
- `initialize` guidance 迁移 TypeScript 版本的主要指令，尤其是优先使用 `codegraph_explore` 和 `codegraph_node`。

## 非首轮范围

- `upgrade` 的完整安装源检测、版本检查、下载安装和重装流程。
- 一次性迁移全部 20+ 语言的 TypeScript 解析质量。
- 一次性迁移全部框架路由和跨语言桥接规则。
- 改写 README、发布脚本或 npm 打包策略，除非实现中发现命令入口需要最小配套调整。

## 架构

Rust CLI 保持 `clap` 作为参数入口，但 `src/main.rs` 不再承载大量业务逻辑。命令实现拆到 `src/cli`，服务能力拆到可复用模块。

建议模块边界：

- `cli`：解析后的命令分发、退出码、text/json 输出入口。
- `project`：项目根目录解析、向上查找已初始化目录、`.codegraph` 路径、初始化状态判断。
- `indexing`：`init`、`index`、`sync`、`uninit`、`unlock`、`status` 的共用逻辑。
- `query`：搜索、调用关系、影响半径、文件视图、目录结构、affected 分析。
- `context`：`explore` 和 `node` 的格式化上下文构建。
- `mcp`：协议适配和工具注册，不直接包含查询业务逻辑。
- `installer`：agent target 检测、配置写入、marker 指令块、权限 allowlist、卸载回滚。

数据流：

```text
CLI/MCP 请求
  -> project root 解析
  -> 打开或验证 .codegraph/codegraph.db
  -> 必要时执行 sync 或 connect-time catch-up
  -> 调用服务层
  -> text/json formatter
  -> 统一错误映射和退出
```

## 命令行为要求

### 初始化和索引

`init`、`index`、`sync`、`status`、`uninit`、`unlock` 按 TypeScript 行为对齐：

- `init [path]` 默认创建 `.codegraph` 并构建初始索引；保留兼容 `-i/--index`。
- `index [path]` 支持 `--force`、`--quiet`、`--verbose`。
- `sync [path]` 支持 `--quiet`，只处理新增、修改、删除文件。
- `status [path] --json` 输出结构化状态，不混入装饰文本。
- `uninit [path] --force` 删除项目索引目录，交互确认后续再按原体验补齐。
- `unlock [path]` 只移除 stale lock，不掩盖真实数据库写入冲突。

### 查询类命令

`query`、`callers`、`callees`、`impact` 要复用同一套查找和消歧逻辑：

- 支持 Windows 反斜杠路径、相对路径、从子目录向上查找项目。
- 支持 `--json` 输出且格式稳定。
- 同名符号按 TypeScript 版分组或消歧，不把无关定义合并成一个结果。
- 找不到符号时给出可操作提示。

### `explore`

`explore` 是首轮的核心迁移对象。它应提供和 `codegraph_explore` MCP 工具一致的语义：

- 接受自然语言或符号名组合。
- 返回相关符号源代码、文件分组、关系图、调用路径和 blast radius。
- 控制输出预算，避免一次返回过大。
- 对被裁剪内容给出后续 `codegraph_explore` 或 `codegraph_node` 指引。
- 不要求用户先运行 `query`。

### `node`

`node` 需要支持两种模式：

- 符号模式：返回符号位置、签名、源码、caller/callee trail；同名符号可返回多个定义或用 `--file`/`--line` 消歧。
- 文件模式：按路径或 basename 读取已索引源文件，输出带行号的源码，支持 `--offset`、`--limit`、`--symbols-only`，并附带依赖方提示。

### `files`

`files` 对齐原版 tree、flat、grouped 输出：

- 支持 `--filter`、`--pattern`、`--format`、`--max-depth`、`--no-metadata`、`--json`。
- tree 输出保持稳定排序。
- JSON 输出包含文件路径、语言、符号数量等结构化字段。

### `affected`

`affected` 迁移 import dependency 追踪：

- 支持参数文件列表和 `--stdin`。
- 支持 `--depth`、`--filter`、`--quiet`、`--json`。
- 自动识别常见测试文件，允许用户用 glob 覆盖。
- 无受影响测试时 text 和 quiet 输出都保持脚本友好。

### `install` 和 `uninstall`

安装器首轮优先保证非交互路径可靠：

- `--yes`
- `--target`
- `--location`
- `--print-config`
- `--no-permissions`

安装器写入应只发生在目标配置路径，测试必须使用临时 HOME、APPDATA 或 XDG 路径。首轮迁移 Claude Code、Cursor、Codex CLI、opencode、Hermes Agent、Gemini CLI、Antigravity IDE、Kiro 的目标识别和 MCP 配置写入语义。交互式提示可在非交互路径稳定后补齐。

## MCP 行为要求

MCP 层必须和 CLI 共用查询服务，不重新实现业务逻辑。

要求：

- 工具 schema 名称、参数名和描述按 TypeScript 版迁移。
- 默认工具集合为 `explore,node,search,callers`。
- `CODEGRAPH_MCP_TOOLS` 支持短名和 `codegraph_` 前缀。
- 未初始化时不列工具。
- `codegraph_node` 文件读取模式输出应和 CLI `node --file` 一致。
- `codegraph_explore` 输出应和 CLI `explore` 一致。
- watcher pending 文件需要在相关响应里显示 staleness banner。

## 错误处理

统一错误策略：

- 未初始化：CLI 明确提示运行 `codegraph init`；MCP inactive 且不暴露工具。
- 数据库错误：保留 SQLite 或 IO 原始原因，不吞掉 `database is locked`、权限错误或迁移失败。
- 路径错误：提示解析后的路径和失败原因。
- JSON 模式：只输出 JSON，不混入进度条、ANSI 或提示文本。
- stdin 模式：输入为空时返回可脚本处理的空结果，不挂起。
- 安装器：写配置失败必须报告目标文件路径和失败原因。

## 测试策略

测试按行为面组织：

- CLI 参数和输出测试：每个子命令覆盖 text 和 JSON。
- MCP 协议测试：initialize、tools/list、tools/call、未初始化、allowlist。
- 路径测试：Windows 反斜杠、相对路径、空格路径、子目录向上查找。
- 文件视图测试：`node` 的 offset、limit、symbols-only、basename 消歧。
- affected 测试：参数列表、stdin、quiet、json、filter。
- 安装器测试：临时 HOME/APPDATA/XDG，验证写入内容和卸载回滚。
- 回归测试：优先复用现有 TypeScript 测试用例作为迁移清单。

首轮验收标准：

1. 首轮范围内所有子命令的帮助、参数和关键输出与 TypeScript 版一致或有明确兼容说明。
2. MCP 默认工具面和未初始化行为与 TypeScript 版一致。
3. `explore`、`node`、`files`、`affected` 有覆盖正常路径和错误路径的测试。
4. 安装器非交互路径在临时配置目录中可验证。
5. `cargo test` 通过，必要时补充 CLI 集成测试命令。

## 后续阶段

后续按独立切片迁移：

1. `upgrade` 完整实现。
2. 全语言 tree-sitter extractor 迁移。
3. 框架路由解析迁移。
4. React Native、Swift/ObjC、Expo、Fabric 等跨语言桥接迁移。
5. README、安装脚本、发布打包和版本号策略对齐 Rust 发行形态。

