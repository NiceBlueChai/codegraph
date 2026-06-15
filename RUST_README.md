# CodeGraph - Rust Implementation

Supercharge Claude Code with semantic code intelligence - Rust implementation.

## Overview

CodeGraph is a high-performance code analysis tool that provides semantic code intelligence by building and querying a graph database of your codebase. It extracts code structures (functions, classes, interfaces, etc.) and their relationships (calls, extends, imports) to enable powerful code navigation and analysis.

## Features

- **Multi-language Support**: TypeScript, JavaScript, Python, Rust
- **Tree-sitter AST Parsing**: Accurate code extraction using tree-sitter grammars
- **Parallel Indexing**: Fast indexing using rayon for parallel file processing
- **Incremental Sync**: Smart change detection with content hashing
- **MCP Protocol**: Model Context Protocol server for AI integration
- **Graph Queries**: Callers, callees, impact analysis, path finding
- **File Watching**: Real-time monitoring with automatic re-indexing

## Installation

```bash
# Build from source
cd codegraph-rust
cargo build --release

# The binary will be in target/release/codegraph
```

## Usage

### Index a Project

```bash
# Full index of current directory
codegraph index

# Index a specific directory
codegraph index /path/to/project
```

### Sync Changes

```bash
# Incremental sync (only process changed files)
codegraph sync
```

### Start MCP Server

```bash
# Start MCP server (stdio mode)
codegraph mcp

# Start MCP daemon (TCP mode)
codegraph daemon --port 9540

# Start MCP proxy (connect to daemon)
codegraph proxy --addr 127.0.0.1:9540
```

### Watch for Changes

```bash
# Watch for file changes and auto-sync
codegraph watch
```

### Query the Graph

```bash
# Find callers of a function
codegraph callers functionName

# Find callees of a function
codegraph callees functionName

# Impact analysis
codegraph impact functionName

# Search for symbols
codegraph search "pattern"

# Show statistics
codegraph stats
```

## Architecture

```
codegraph-rust/
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs               # Library root
│   ├── types.rs             # Core data types
│   ├── db/
│   │   ├── connection.rs    # Database connection
│   │   ├── schema.rs        # Schema initialization
│   │   └── queries.rs       # Query builder
│   ├── extraction/
│   │   ├── parser.rs        # Text-based parser (fallback)
│   │   ├── tree_sitter_parser.rs  # Tree-sitter AST parser
│   │   └── languages/       # Language-specific extractors
│   ├── core/
│   │   ├── indexer.rs        # Parallel indexing engine
│   │   ├── resolver.rs       # Reference resolver
│   │   └── query.rs          # Graph traversal
│   ├── mcp/
│   │   ├── protocol.rs       # JSON-RPC protocol
│   │   ├── transport.rs      # Stdio/TCP transport
│   │   ├── server.rs         # MCP server
│   │   └── tools.rs          # Tool implementations
│   └── sync/
│       └── watcher.rs        # File watcher
└── Cargo.toml
```

## MCP Tools

The MCP server exposes the following tools:

| Tool | Description |
|------|-------------|
| `codegraph_query` | Query nodes by name or pattern |
| `codegraph_callers` | Find all callers of a function |
| `codegraph_callees` | Find all callees of a function |
| `codegraph_impact` | Impact analysis for a function |
| `codegraph_search` | Search for symbols |
| `codegraph_stats` | Get database statistics |

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `CODEGRAPH_DB_PATH` | `.codegraph/codegraph.db` | Database file path |
| `CODEGRAPH_WATCH_DEBOUNCE_MS` | `2000` | Watch debounce interval |
| `CODEGRAPH_LOG_LEVEL` | `info` | Log level (trace/debug/info/warn/error) |

## Performance

- **Parallel Indexing**: Uses rayon for multi-threaded file parsing
- **Incremental Sync**: Only processes files with changed content hash
- **Tree-sitter**: Fast AST parsing with accurate code extraction
- **SQLite**: Embedded database with optimized queries

## Development

```bash
# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run -- index

# Build release
cargo build --release
```

## License

MIT
