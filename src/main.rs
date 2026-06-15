use clap::{Parser, Subcommand};
use codegraph::db::{DatabaseConnection, QueryBuilder};
use codegraph::db::schema::initialize_schema;
use log::info;

#[derive(Parser)]
#[command(name = "codegraph")]
#[command(about = "Supercharge Claude Code with semantic code intelligence - Rust version", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new CodeGraph project
    Init {
        /// Project root directory
        #[arg(default_value = ".")]
        path: String,

        /// Show detailed worker lifecycle and memory info
        #[arg(short, long)]
        verbose: bool,
    },

    /// Index the project
    Index {
        /// Project root directory
        #[arg(default_value = ".")]
        path: String,

        /// Force full re-index (clear existing data)
        #[arg(short, long)]
        force: bool,

        /// Suppress progress output
        #[arg(short, long)]
        quiet: bool,

        /// Show detailed worker lifecycle and memory info
        #[arg(short, long)]
        verbose: bool,
    },

    /// Sync changes
    Sync {
        /// Project root directory
        #[arg(default_value = ".")]
        path: String,

        /// Suppress output (for git hooks)
        #[arg(short, long)]
        quiet: bool,
    },

    /// Show project status
    Status {
        /// Project root directory
        #[arg(default_value = ".")]
        path: String,

        /// Output as JSON
        #[arg(short, long)]
        json: bool,
    },

    /// Query symbols
    Query {
        /// Search query
        query: String,

        /// Project root directory
        #[arg(short, long, default_value = ".")]
        path: String,

        /// Maximum results
        #[arg(short, long, default_value_t = 10)]
        limit: usize,

        /// Filter by node kind (function, class, etc.)
        #[arg(short = 'k', long)]
        kind: Option<String>,

        /// Output as JSON
        #[arg(short, long)]
        json: bool,
    },

    /// Start MCP server
    Serve {
        /// Project root directory (MCP mode uses client's rootUri)
        #[arg(short, long)]
        path: Option<String>,

        /// Run in MCP mode (stdio transport)
        #[arg(long)]
        mcp: bool,

        /// Disable file watcher (no auto-sync; slower filesystems like WSL2 /mnt)
        #[arg(long)]
        no_watch: bool,
    },

    /// Find callers of a symbol
    Callers {
        /// Symbol name
        symbol: String,

        /// Project root directory
        #[arg(short, long, default_value = ".")]
        path: String,

        /// Maximum results
        #[arg(short, long, default_value_t = 20)]
        limit: usize,

        /// Output as JSON
        #[arg(short, long)]
        json: bool,
    },

    /// Find callees of a symbol
    Callees {
        /// Symbol name
        symbol: String,

        /// Project root directory
        #[arg(short, long, default_value = ".")]
        path: String,

        /// Maximum results
        #[arg(short, long, default_value_t = 20)]
        limit: usize,

        /// Output as JSON
        #[arg(short, long)]
        json: bool,
    },

    /// Impact analysis for a symbol
    Impact {
        /// Symbol name
        symbol: String,

        /// Project root directory
        #[arg(short, long, default_value = ".")]
        path: String,

        /// Traversal depth
        #[arg(short, long, default_value_t = 2)]
        depth: usize,

        /// Output as JSON
        #[arg(short, long)]
        json: bool,
    },

    /// Remove CodeGraph from project
    Uninit {
        /// Project root directory
        #[arg(default_value = ".")]
        path: String,

        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Remove stale lock files
    Unlock {
        /// Project root directory
        #[arg(default_value = ".")]
        path: String,
    },

    /// Show project file structure
    Files {
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        path: String,

        /// Filter to directory
        #[arg(long)]
        filter: Option<String>,

        /// Glob pattern filter
        #[arg(long)]
        pattern: Option<String>,

        /// Output format: tree, flat, grouped
        #[arg(long, default_value = "tree")]
        format: String,

        /// Max directory depth for tree format
        #[arg(long)]
        max_depth: Option<usize>,

        /// Output as JSON
        #[arg(short, long)]
        json: bool,

        /// Hide file metadata (language, symbol count)
        #[arg(long)]
        no_metadata: bool,
    },
    Explore {
        /// Query terms
        query: Vec<String>,

        /// Project root directory
        #[arg(short, long, default_value = ".")]
        path: String,

        /// Max files to include source
        #[arg(long, default_value_t = 5)]
        max_files: usize,
    },

    /// Show symbol details or read file
    Node {
        /// Symbol name or file path
        name: String,

        /// Project root directory
        #[arg(short, long, default_value = ".")]
        path: String,

        /// File mode: read this file
        #[arg(short, long)]
        file: Option<String>,

        /// File mode: start line (1-based)
        #[arg(long)]
        offset: Option<usize>,

        /// File mode: max lines
        #[arg(long)]
        limit: Option<usize>,

        /// File mode: only show symbol map
        #[arg(long)]
        symbols_only: bool,
    },

    /// Find affected test files
    Affected {
        /// Changed files
        files: Vec<String>,

        /// Project root directory
        #[arg(short, long, default_value = ".")]
        path: String,

        /// Read file list from stdin
        #[arg(long)]
        stdin: bool,

        /// Max dependency depth
        #[arg(short, long, default_value_t = 5)]
        depth: usize,

        /// Test file glob filter
        #[arg(short, long)]
        filter: Option<String>,

        /// Output as JSON
        #[arg(short, long)]
        json: bool,

        /// Quiet mode: only file paths
        #[arg(short, long)]
        quiet: bool,
    },

    /// Install MCP server to AI agents
    Install {
        /// Target agents (comma-separated or "auto"/"all"/"none")
        #[arg(short, long)]
        target: Option<String>,

        /// Installation location: global or local
        #[arg(short, long)]
        location: Option<String>,

        /// Non-interactive mode
        #[arg(short, long)]
        yes: bool,

        /// Skip permissions (Claude Code only)
        #[arg(long)]
        no_permissions: bool,

        /// Print config for agent and exit
        #[arg(long)]
        print_config: Option<String>,
    },

    /// Uninstall MCP server from AI agents
    Uninstall {
        /// Target agents (comma-separated or "all")
        #[arg(short, long)]
        target: Option<String>,

        /// Uninstall location: global or local
        #[arg(short, long)]
        location: Option<String>,

        /// Non-interactive mode
        #[arg(short, long)]
        yes: bool,
    },

    /// Upgrade CodeGraph
    Upgrade {
        /// Target version
        version: Option<String>,

        /// Only check for updates
        #[arg(long)]
        check: bool,

        /// Force reinstall
        #[arg(short, long)]
        force: bool,
    },
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Init { path, verbose }) => {
            info!("Initializing CodeGraph project at {}", path);
            cmd_init(&path, verbose)?;
        }
        Some(Commands::Index { path, force, quiet, verbose }) => {
            info!("Indexing project at {}", path);
            cmd_index(&path, force, quiet, verbose)?;
        }
        Some(Commands::Sync { path, quiet }) => {
            info!("Syncing changes at {}", path);
            cmd_sync(&path, quiet)?;
        }
        Some(Commands::Status { path, json }) => {
            info!("Checking status at {}", path);
            cmd_status(&path, json)?;
        }
        Some(Commands::Query { query, path, limit, kind, json }) => {
            info!("Querying at {}: {}", path, query);
            cmd_query(&path, &query, limit, kind.as_deref(), json)?;
        }
        Some(Commands::Serve { path, mcp, no_watch }) => {
            info!("Starting MCP server (mcp={})", mcp);
            cmd_serve(path.as_deref(), mcp, no_watch)?;
        }
        Some(Commands::Callers { symbol, path, limit, json }) => {
            info!("Finding callers of '{}' at {}", symbol, path);
            cmd_callers(&path, &symbol, limit, json)?;
        }
        Some(Commands::Callees { symbol, path, limit, json }) => {
            info!("Finding callees of '{}' at {}", symbol, path);
            cmd_callees(&path, &symbol, limit, json)?;
        }
        Some(Commands::Impact { symbol, path, depth, json }) => {
            info!("Analyzing impact of '{}' at {}", symbol, path);
            cmd_impact(&path, &symbol, depth, json)?;
        }
        Some(Commands::Uninit { path, force }) => {
            info!("Uninitializing project at {}", path);
            cmd_uninit(&path, force)?;
        }
        Some(Commands::Unlock { path }) => {
            info!("Unlocking project at {}", path);
            cmd_unlock(&path)?;
        }
        Some(Commands::Files { path, filter, pattern, format, max_depth, json, no_metadata }) => {
            info!("Showing files at {}", path);
            cmd_files(&path, filter.as_deref(), pattern.as_deref(), &format, max_depth, json, no_metadata)?;
        }
        Some(Commands::Explore { query, path, max_files }) => {
            info!("Exploring at {}: {:?}", path, query);
            cmd_explore(&path, &query, max_files)?;
        }
        Some(Commands::Node { name, path, file, offset, limit, symbols_only }) => {
            info!("Node at {}: {}", path, name);
            cmd_node(&path, &name, file.as_deref(), offset, limit, symbols_only)?;
        }
        Some(Commands::Affected { files, path, stdin, depth, filter, json, quiet }) => {
            info!("Finding affected files at {}", path);
            cmd_affected(&path, &files, stdin, depth, filter.as_deref(), json, quiet)?;
        }
        Some(Commands::Install { target, location, yes, no_permissions, print_config }) => {
            info!("Installing MCP server");
            cmd_install(target.as_deref(), location.as_deref(), yes, no_permissions, print_config.as_deref())?;
        }
        Some(Commands::Uninstall { target, location, yes }) => {
            info!("Uninstalling MCP server");
            cmd_uninstall(target.as_deref(), location.as_deref(), yes)?;
        }
        Some(Commands::Upgrade { version, check, force }) => {
            info!("Upgrading CodeGraph");
            cmd_upgrade(version.as_deref(), check, force)?;
        }
        None => {
            println!("CodeGraph Rust v{}", env!("CARGO_PKG_VERSION"));
            println!("Use --help to see available commands");
        }
    }

    Ok(())
}

fn cmd_init(path: &str, verbose: bool) -> anyhow::Result<()> {
    use codegraph::db::{get_database_path, create_directory};

    if verbose {
        eprintln!("  Initializing at {}...", path);
    }

    create_directory(path)?;

    let db_path = get_database_path(path);
    let db = DatabaseConnection::initialize(&db_path)
        .map_err(|e| {
            eprintln!("Database initialization error: {:?}", e);
            anyhow::anyhow!("Failed to initialize database: {}", e)
        })?;
    initialize_schema(db.get_conn())?;

    println!("✓ CodeGraph initialized in {}", path);
    println!("  Database: {}", db_path);

    // Auto-index after init (matching TS behavior)
    println!();
    cmd_index(path, false, false, verbose)?;

    Ok(())
}

fn cmd_index(path: &str, force: bool, quiet: bool, verbose: bool) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;
    use codegraph::core::indexer::Indexer;

    if !codegraph::db::is_initialized(path) {
        anyhow::bail!("CodeGraph not initialized. Run 'codegraph init' first.");
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("Failed to open database: {}", e))?;
    initialize_schema(db.get_conn())?;

    // Force full re-index by clearing existing data
    if force {
        if !quiet {
            println!("Forcing full re-index (clearing existing data)...");
        }
        let queries = QueryBuilder::new(db.get_conn());
        queries.clear().map_err(|e| anyhow::anyhow!("Failed to clear data: {}", e))?;
    }

    let queries = QueryBuilder::new(db.get_conn());
    let indexer = Indexer::new(queries, path);

    if !quiet {
        println!("Indexing...");
    }
    let result = indexer.index_all().map_err(|e| anyhow::anyhow!("Indexing failed: {}", e))?;
    if !quiet {
        println!("✓ Indexing complete");
        println!("  Files indexed: {}", result.files_indexed);
        println!("  Files skipped: {}", result.files_skipped);
        println!("  Nodes created: {}", result.nodes_created);
        println!("  Edges created: {}", result.edges_created);
    }

    // Verbose: show detailed per-file error info and timing
    if verbose && !result.errors.is_empty() {
        eprintln!("\n  Errors ({}):", result.errors.len());
        for err in &result.errors {
            if let Some(ref file) = err.file_path {
                eprintln!("    [{}] {}: {}", err.severity, file, err.message);
            } else {
                eprintln!("    [{}] {}", err.severity, err.message);
            }
        }
    }
    if verbose {
        let parsed = result.files_indexed + result.files_errored;
        let rate = if result.duration_ms > 0 {
            (parsed as f64) / (result.duration_ms as f64 / 1000.0)
        } else {
            0.0
        };
        eprintln!("  Duration: {}ms ({:.1} files/s)", result.duration_ms, rate);
    }

    Ok(())
}

fn cmd_sync(path: &str, quiet: bool) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;
    use codegraph::core::indexer::Indexer;

    if !codegraph::db::is_initialized(path) {
        anyhow::bail!("CodeGraph not initialized. Run 'codegraph init' first.");
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("Failed to open database: {}", e))?;

    if !quiet {
        println!("Syncing...");
    }
    let indexer = Indexer::new(QueryBuilder::new(db.get_conn()), path);
    let result = indexer.sync().map_err(|e| anyhow::anyhow!("Sync failed: {}", e))?;

    if !quiet {
        println!("✓ Sync complete");
        println!("  Files checked: {}", result.files_checked);
        println!("  Files added: {}", result.files_added);
        println!("  Files modified: {}", result.files_modified);
        println!("  Duration: {}ms", result.duration_ms);
    }

    Ok(())
}

fn cmd_status(path: &str, json: bool) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;
    use std::fs;

    if !codegraph::db::is_initialized(path) {
        if json {
            println!("{{\"initialized\": false}}");
        } else {
            println!("CodeGraph not initialized");
        }
        return Ok(());
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("Failed to open database: {}", e))?;
    let queries = QueryBuilder::new(db.get_conn());
    let stats = queries.get_stats()?;
    let nodes_by_kind = queries.get_nodes_by_kind_counts().unwrap_or_default();
    let languages: Vec<String> = queries.get_languages().unwrap_or_default();
    let files_by_lang = queries.get_file_counts_by_language().unwrap_or_default();

    // Get DB file size
    let db_size_bytes = fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    if json {
        let output = serde_json::json!({
            "initialized": true,
            "version": env!("CARGO_PKG_VERSION"),
            "backend": format!("{:?}", db.get_backend()),
            "journal_mode": db.get_journal_mode().unwrap_or_else(|_| "unknown".to_string()),
            "nodes": stats.node_count,
            "edges": stats.edge_count,
            "files": stats.file_count,
            "unresolved_refs": stats.unresolved_ref_count,
            "db_size_bytes": db_size_bytes,
            "nodes_by_kind": nodes_by_kind.iter().map(|(k, c)| serde_json::json!({"kind": k, "count": c})).collect::<Vec<_>>(),
            "languages": languages,
            "files_by_language": files_by_lang.iter().map(|(l, c)| serde_json::json!({"language": l, "count": c})).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("CodeGraph Status:");
        println!("  Version: {}", env!("CARGO_PKG_VERSION"));
        println!("  Backend: {:?}", db.get_backend());
        println!("  Journal Mode: {}", db.get_journal_mode().unwrap_or_else(|_| "unknown".to_string()));
        println!("  Nodes: {}", stats.node_count);
        println!("  Edges: {}", stats.edge_count);
        println!("  Files: {}", stats.file_count);
        println!("  Unresolved Refs: {}", stats.unresolved_ref_count);
        println!("  DB Size: {:.1} MB", db_size_bytes as f64 / 1_048_576.0);

        // Nodes by kind
        if !nodes_by_kind.is_empty() {
            println!("\n  Nodes by kind:");
            for (kind, count) in &nodes_by_kind {
                println!("    {:15} {}", format!("{}:", kind), count);
            }
        }

        // Files by language
        if !files_by_lang.is_empty() {
            println!("\n  Files by language:");
            for (lang, count) in &files_by_lang {
                println!("    {:15} {}", format!("{}:", lang), count);
            }
        }
    }

    Ok(())
}

fn cmd_query(path: &str, query: &str, limit: usize, kind: Option<&str>, json: bool) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;

    if !codegraph::db::is_initialized(path) {
        anyhow::bail!("CodeGraph not initialized");
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("Failed to open database: {}", e))?;
    let queries = QueryBuilder::new(db.get_conn());

    let options = codegraph::types::SearchOptions {
        limit,
        kinds: kind.map(|k| {
            k.split(',')
                .filter_map(|s| codegraph::types::NodeKind::from_str(s.trim()))
                .collect()
        }),
        file_pattern: None,
    };
    let results = queries.search_nodes(query, Some(&options))?;

    if json {
        let output = serde_json::json!({
            "query": query,
            "results": results.iter().take(limit).map(|r| {
                serde_json::json!({
                    "name": r.node.name,
                    "kind": r.node.kind.as_str(),
                    "file": r.node.file_path,
                    "line": r.node.start_line,
                    "signature": r.node.signature,
                })
            }).collect::<Vec<_>>(),
            "total": results.len(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if results.is_empty() {
        println!("No results found for '{}'", query);
    } else {
        println!("Found {} results for '{}':", results.len(), query);
        for result in results.iter().take(limit) {
            println!("  - {} ({}) in {}", result.node.name, result.node.kind.as_str(), result.node.file_path);
            if let Some(ref sig) = result.node.signature {
                println!("    Signature: {}", sig);
            }
        }
    }

    Ok(())
}

fn cmd_serve(path: Option<&str>, mcp: bool, no_watch: bool) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;

    if mcp {
        // Use specified path or current directory
        let project_root = path.unwrap_or(".");

        if !codegraph::db::is_initialized(project_root) {
            anyhow::bail!("CodeGraph not initialized. Run 'codegraph init' first.");
        }

        let db_path = get_database_path(project_root);
        let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("Failed to open database: {}", e))?;
        let queries = QueryBuilder::new(db.get_conn());

        let mut server = codegraph::mcp::server::MCPServer::new().with_queries(queries);
        server.run().map_err(|e| anyhow::anyhow!("MCP server error: {}", e))?;
    } else {
        println!("Starting MCP server...");
        println!("Use --mcp flag to start in MCP mode");
        if no_watch {
            println!("  File watcher: disabled");
        }
    }

    Ok(())
}

fn cmd_callers(path: &str, symbol: &str, limit: usize, json: bool) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;
    use codegraph::core::query::GraphTraverser;
    use std::collections::HashSet;

    if !codegraph::db::is_initialized(path) {
        anyhow::bail!("CodeGraph not initialized");
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("{}", e))?;
    let queries = QueryBuilder::new(db.get_conn());

    // Search for symbol first
    let options = codegraph::types::SearchOptions { limit: 50, kinds: None, file_pattern: None };
    let results = queries.search_nodes(symbol, Some(&options))?;

    // Filter to exact name matches (exact, :: suffix, or . suffix)
    let exact_matches: Vec<_> = results.iter().filter(|r| {
        r.node.name == symbol
            || r.node.qualified_name.ends_with(&format!("::{}", symbol))
            || r.node.name.ends_with(&format!(".{}", symbol))
    }).collect();

    // Fall back to top match if exact filter removes everything
    let matches: Vec<_> = if exact_matches.is_empty() {
        results.iter().take(1).collect()
    } else {
        exact_matches
    };

    if matches.is_empty() {
        println!("No matching symbols found for '{}'", symbol);
        return Ok(());
    }

    // Get callers with deduplication
    let traverser = GraphTraverser { queries };
    let mut seen: HashSet<String> = HashSet::new();
    let mut all_callers: Vec<(codegraph::types::Node, codegraph::types::Edge)> = Vec::new();

    for result in &matches {
        let callers = traverser.get_callers(&result.node.id, 1).map_err(|e| anyhow::anyhow!("{}", e))?;
        for (node, edge) in callers {
            if seen.insert(node.id.clone()) {
                all_callers.push((node, edge));
            }
        }
    }

    if json {
        let output = serde_json::json!({
            "symbol": symbol,
            "callers": all_callers.iter().take(limit).map(|(node, edge)| {
                serde_json::json!({
                    "name": node.name,
                    "filePath": node.file_path,
                    "startLine": node.start_line,
                    "kind": node.kind.as_str(),
                    "edgeKind": edge.kind.as_str(),
                })
            }).collect::<Vec<_>>(),
            "total": all_callers.len(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        if all_callers.is_empty() {
            println!("No callers found for '{}'", symbol);
        } else {
            println!("Callers of '{}' ({}):", symbol, all_callers.len());
            for (node, _) in all_callers.iter().take(limit) {
                println!("  - {} ({}) at {}:{}", node.name, node.kind.as_str(), node.file_path, node.start_line);
            }
        }
    }

    Ok(())
}

fn cmd_callees(path: &str, symbol: &str, limit: usize, json: bool) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;
    use codegraph::core::query::GraphTraverser;
    use std::collections::HashSet;

    if !codegraph::db::is_initialized(path) {
        anyhow::bail!("CodeGraph not initialized");
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("{}", e))?;
    let queries = QueryBuilder::new(db.get_conn());

    // Search for symbol first
    let options = codegraph::types::SearchOptions { limit: 50, kinds: None, file_pattern: None };
    let results = queries.search_nodes(symbol, Some(&options))?;

    // Filter to exact name matches (exact, :: suffix, or . suffix)
    let exact_matches: Vec<_> = results.iter().filter(|r| {
        r.node.name == symbol
            || r.node.qualified_name.ends_with(&format!("::{}", symbol))
            || r.node.name.ends_with(&format!(".{}", symbol))
    }).collect();

    let matches: Vec<_> = if exact_matches.is_empty() {
        results.iter().take(1).collect()
    } else {
        exact_matches
    };

    if matches.is_empty() {
        println!("No matching symbols found for '{}'", symbol);
        return Ok(());
    }

    // Get callees with deduplication
    let traverser = GraphTraverser { queries };
    let mut seen: HashSet<String> = HashSet::new();
    let mut all_callees: Vec<(codegraph::types::Node, codegraph::types::Edge)> = Vec::new();

    for result in &matches {
        let callees = traverser.get_callees(&result.node.id, 1).map_err(|e| anyhow::anyhow!("{}", e))?;
        for (node, edge) in callees {
            if seen.insert(node.id.clone()) {
                all_callees.push((node, edge));
            }
        }
    }

    if json {
        let output = serde_json::json!({
            "symbol": symbol,
            "callees": all_callees.iter().take(limit).map(|(node, edge)| {
                serde_json::json!({
                    "name": node.name,
                    "filePath": node.file_path,
                    "startLine": node.start_line,
                    "kind": node.kind.as_str(),
                    "edgeKind": edge.kind.as_str(),
                })
            }).collect::<Vec<_>>(),
            "total": all_callees.len(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        if all_callees.is_empty() {
            println!("No callees found for '{}'", symbol);
        } else {
            println!("Callees of '{}' ({}):", symbol, all_callees.len());
            for (node, _) in all_callees.iter().take(limit) {
                println!("  - {} ({}) at {}:{}", node.name, node.kind.as_str(), node.file_path, node.start_line);
            }
        }
    }

    Ok(())
}

fn cmd_impact(path: &str, symbol: &str, depth: usize, json: bool) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;
    use codegraph::core::query::GraphTraverser;
    use std::collections::{HashSet, BTreeMap};

    // Clamp depth to 1-10 range (matching TS behavior)
    let depth = depth.max(1).min(10);

    if !codegraph::db::is_initialized(path) {
        anyhow::bail!("CodeGraph not initialized");
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("{}", e))?;
    let queries = QueryBuilder::new(db.get_conn());

    // Search for symbol first
    let options = codegraph::types::SearchOptions { limit: 50, kinds: None, file_pattern: None };
    let results = queries.search_nodes(symbol, Some(&options))?;

    // Filter to exact name matches
    let exact_matches: Vec<_> = results.iter().filter(|r| {
        r.node.name == symbol
            || r.node.qualified_name.ends_with(&format!("::{}", symbol))
            || r.node.name.ends_with(&format!(".{}", symbol))
    }).collect();

    let matches: Vec<_> = if exact_matches.is_empty() {
        results.iter().take(1).collect()
    } else {
        exact_matches
    };

    if matches.is_empty() {
        println!("No matching symbols found for '{}'", symbol);
        return Ok(());
    }

    // Merge impact subgraphs from all matching symbols
    let traverser = GraphTraverser { queries };
    let mut merged = codegraph::types::Subgraph::new();
    let mut edge_seen: HashSet<(String, String, String)> = HashSet::new();

    for result in &matches {
        let impact = traverser.get_impact_radius(&result.node.id, depth)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        for (id, node) in impact.nodes {
            merged.nodes.entry(id).or_insert(node);
        }
        for edge in impact.edges {
            let key = (edge.source.clone(), edge.target.clone(), edge.kind.as_str().to_string());
            if edge_seen.insert(key) {
                merged.edges.push(edge);
            }
        }
    }

    // Collect nodes (exclude the symbol nodes themselves)
    let match_ids: HashSet<&str> = matches.iter().map(|r| r.node.id.as_str()).collect();
    let affected_nodes: Vec<&codegraph::types::Node> = merged.nodes.values()
        .filter(|n| !match_ids.contains(n.id.as_str()))
        .collect();

    if json {
        let output = serde_json::json!({
            "symbol": symbol,
            "depth": depth,
            "nodeCount": affected_nodes.len(),
            "edgeCount": merged.edges.len(),
            "affected": affected_nodes.iter().map(|node| {
                serde_json::json!({
                    "name": node.name,
                    "filePath": node.file_path,
                    "startLine": node.start_line,
                    "kind": node.kind.as_str(),
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        if affected_nodes.is_empty() {
            println!("No impact found for '{}'", symbol);
        } else {
            // Group by file
            let mut by_file: BTreeMap<&str, Vec<&codegraph::types::Node>> = BTreeMap::new();
            for node in &affected_nodes {
                by_file.entry(node.file_path.as_str()).or_default().push(node);
            }
            println!("Impact of changing \"{}\" — {} affected symbols:", symbol, affected_nodes.len());
            for (file, nodes) in &by_file {
                println!("  {}:", file);
                for node in nodes {
                    println!("    {:12} {}", format!("({})", node.kind.as_str()), node.name);
                }
            }
        }
    }

    Ok(())
}

fn cmd_uninit(path: &str, force: bool) -> anyhow::Result<()> {
    use std::fs;
    use std::path::Path;

    let codegraph_dir = Path::new(path).join(".codegraph");

    if !codegraph_dir.exists() {
        println!("CodeGraph not initialized in {}", path);
        return Ok(());
    }

    if !force {
        eprintln!(
            "⚠ This will permanently remove the .codegraph/ directory in '{}'\n\
             \x20  including the database and all indexed data.\n\
             \x20  Run 'codegraph uninit --force' to confirm.",
            path
        );
        return Ok(());
    }

    fs::remove_dir_all(&codegraph_dir)?;
    println!("✓ CodeGraph removed from {}", path);

    Ok(())
}

fn cmd_unlock(path: &str) -> anyhow::Result<()> {
    use std::fs;
    use std::path::Path;
    use globset::Glob;

    let codegraph_dir = Path::new(path).join(".codegraph");

    if !codegraph_dir.exists() {
        println!("CodeGraph not initialized in {}", path);
        return Ok(());
    }

    let glob = Glob::new("*.lock")?.compile_matcher();
    let mut removed = 0;

    for entry in fs::read_dir(&codegraph_dir)? {
        let entry = entry?;
        let file_name = entry.file_name();
        if glob.is_match(&file_name) {
            fs::remove_file(entry.path())?;
            removed += 1;
            println!("  Removed: {}", file_name.to_string_lossy());
        }
    }

    if removed == 0 {
        println!("No lock files found");
    } else {
        println!("✓ Removed {} lock file(s)", removed);
    }

    Ok(())
}

fn cmd_files(
    path: &str,
    filter: Option<&str>,
    pattern: Option<&str>,
    format: &str,
    max_depth: Option<usize>,
    json: bool,
    no_metadata: bool,
) -> anyhow::Result<()> {
    use std::collections::BTreeMap;
    use globset::Glob;

    if !codegraph::db::is_initialized(path) {
        anyhow::bail!("CodeGraph not initialized. Run 'codegraph init' first.");
    }

    let db_path = codegraph::db::get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("{}", e))?;
    let queries = QueryBuilder::new(db.get_conn());

    // Get files from database
    let all_files = queries.get_all_files()?;

    // Apply filters
    let pattern_matcher = if let Some(pat) = pattern {
        Some(Glob::new(pat)?.compile_matcher())
    } else {
        None
    };

    let mut files: Vec<&codegraph::types::FileRecord> = all_files.iter().filter(|f| {
        // Apply directory filter
        if let Some(filter_dir) = filter {
            if !f.path.starts_with(filter_dir) {
                return false;
            }
        }
        // Apply pattern filter
        if let Some(ref glob) = pattern_matcher {
            let file_name = std::path::Path::new(&f.path).file_name().unwrap_or_default();
            if !glob.is_match(file_name) {
                return false;
            }
        }
        true
    }).collect();

    files.sort_by(|a, b| a.path.cmp(&b.path));

    if json {
        let files_json: Vec<_> = files.iter().map(|f| {
            let mut obj = serde_json::json!({
                "path": f.path,
            });
            if !no_metadata {
                obj["language"] = serde_json::json!(f.language.as_str());
                obj["node_count"] = serde_json::json!(f.node_count);
                obj["size"] = serde_json::json!(f.size);
            }
            obj
        }).collect();
        let output = serde_json::json!({
            "root": path,
            "files": files_json,
            "count": files.len(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    match format {
        "flat" => {
            for f in &files {
                if no_metadata {
                    println!("{}", f.path);
                } else {
                    println!("{} ({}, {} symbols)", f.path, f.language.as_str(), f.node_count);
                }
            }
        }
        "grouped" => {
            let mut by_lang: BTreeMap<String, Vec<&codegraph::types::FileRecord>> = BTreeMap::new();
            for f in &files {
                by_lang.entry(f.language.as_str().to_string()).or_default().push(f);
            }
            for (lang, lang_files) in &by_lang {
                println!("{} ({} files):", lang, lang_files.len());
                for f in lang_files {
                    if no_metadata {
                        println!("  {}", f.path);
                    } else {
                        println!("  {} ({} symbols)", f.path, f.node_count);
                    }
                }
            }
        }
        _ => {
            // Tree format: build directory tree from file paths
            println!("{}", path);
            build_file_tree(&files, max_depth, no_metadata, 1);
        }
    }

    println!("\n{} files", files.len());
    Ok(())
}

/// Build and print a directory tree from file records
fn build_file_tree(
    files: &[&codegraph::types::FileRecord],
    max_depth: Option<usize>,
    no_metadata: bool,
    current_depth: usize,
) {
    use std::collections::BTreeMap;

    // Group files by their top-level directory component
    let mut groups: BTreeMap<String, Vec<&codegraph::types::FileRecord>> = BTreeMap::new();

    for &f in files {
        let parts: Vec<&str> = f.path.split('/').collect();
        if parts.is_empty() { continue; }

        let key = if parts.len() == 1 {
            // File in root
            String::new()
        } else {
            parts[0].to_string()
        };
        groups.entry(key).or_default().push(f);
    }

    // Sort entries: directories first, then files
    let mut entries: Vec<(&String, &Vec<&codegraph::types::FileRecord>)> = groups.iter().collect();
    entries.sort_by(|(a, _), (b, _)| {
        let a_is_dir = !a.is_empty();
        let b_is_dir = !b.is_empty();
        b_is_dir.cmp(&a_is_dir).then(a.cmp(b))
    });

    let depth_reached = max_depth.map(|d| current_depth >= d).unwrap_or(false);

    for (i, (dir_name, dir_files)) in entries.iter().enumerate() {
        let is_last = i == entries.len() - 1;
        let prefix = if is_last { "└── " } else { "├── " };
        let indent = if is_last { "    " } else { "│   " };

        if dir_name.is_empty() {
            // Root-level files
            for (j, &f) in dir_files.iter().enumerate() {
                let file_last = j == dir_files.len() - 1 && is_last;
                let file_prefix = if file_last { "└── " } else { "├── " };
                if no_metadata {
                    let name = std::path::Path::new(&f.path).file_name().unwrap_or_default().to_string_lossy();
                    println!("{}{}", file_prefix, name);
                } else {
                    let name = std::path::Path::new(&f.path).file_name().unwrap_or_default().to_string_lossy();
                    println!("{}{} ({}, {} symbols)", file_prefix, name, f.language.as_str(), f.node_count);
                }
            }
        } else {
            // Directory
            println!("{}{}/", prefix, dir_name);

            if !depth_reached {
                // Recurse: strip directory prefix from file paths
                let _sub_files: Vec<&codegraph::types::FileRecord> = dir_files.iter().map(|&f| f).collect();
                // Create copies with directory prefix stripped
                // For simplicity, use the file path as-is and adjust indentation
                for (j, &f) in dir_files.iter().enumerate() {
                    let file_last = j == dir_files.len() - 1;
                    let line_prefix = if file_last { "└── " } else { "├── " };
                    let _full_indent = if is_last { "    " } else { "│   " };
                    let name = std::path::Path::new(&f.path).file_name().unwrap_or_default().to_string_lossy();
                    if f.path.contains('/') {
                        if no_metadata {
                            println!("{}{}{}", indent, line_prefix, &f.path[dir_name.len()+1..]);
                        } else {
                            println!("{}{}{} ({}, {} symbols)", indent, line_prefix,
                                &f.path[dir_name.len()+1..], f.language.as_str(), f.node_count);
                        }
                    } else {
                        if no_metadata {
                            println!("{}{}{}", indent, line_prefix, name);
                        } else {
                            println!("{}{}{} ({}, {} symbols)", indent, line_prefix,
                                name, f.language.as_str(), f.node_count);
                        }
                    }
                }
            }
        }
    }
}

fn cmd_explore(path: &str, query: &[String], max_files: usize) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;
    use codegraph::core::query::GraphTraverser;

    if !codegraph::db::is_initialized(path) {
        anyhow::bail!("CodeGraph not initialized");
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("{}", e))?;
    let queries = QueryBuilder::new(db.get_conn());

    let search_query = query.join(" ");
    let options = codegraph::types::SearchOptions::default();
    let results = queries.search_nodes(&search_query, Some(&options))?;

    if results.is_empty() {
        println!("No results found for '{}'", search_query);
        return Ok(());
    }

    println!("Exploring '{}':\n", search_query);

    let traverser = GraphTraverser { queries };

    let mut files_shown = 0;

    for result in results.iter() {
        if files_shown >= max_files {
            break;
        }

        let node = &result.node;
        println!("=== {} ({}) ===", node.name, node.kind.as_str());
        println!("  File: {}:{}", node.file_path, node.start_line);

        // Show callers and callees
        if let Ok(callers) = traverser.get_callers(&node.id, 1) {
            if !callers.is_empty() {
                println!("  Called by:");
                for (caller, _) in callers.iter().take(5) {
                    println!("    - {} ({}) at {}:{}", caller.name, caller.kind.as_str(), caller.file_path, caller.start_line);
                }
            }
        }
        if let Ok(callees) = traverser.get_callees(&node.id, 1) {
            if !callees.is_empty() {
                println!("  Calls:");
                for (callee, _) in callees.iter().take(5) {
                    println!("    - {} ({}) at {}:{}", callee.name, callee.kind.as_str(), callee.file_path, callee.start_line);
                }
            }
        }

        // Try to read source
        let full_path = std::path::Path::new(path).join(&node.file_path);
        if let Ok(content) = codegraph::util::read_file_content(&full_path) {
            let lines: Vec<&str> = content.lines().collect();
            let start = (node.start_line as usize).saturating_sub(2);
            let end = (node.end_line as usize + 2).min(lines.len());

            println!("  Source (lines {}-{}):", start + 1, end);
            for i in start..end {
                let line_num = i + 1;
                let marker = if line_num == node.start_line as usize { ">" } else { " " };
                println!("  {} {:4} | {}", marker, line_num, lines[i]);
            }
            files_shown += 1;
        }
        println!();
    }

    Ok(())
}

fn cmd_node(
    path: &str,
    name: &str,
    file: Option<&str>,
    offset: Option<usize>,
    limit: Option<usize>,
    symbols_only: bool,
) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;
    use std::path::Path;

    if !codegraph::db::is_initialized(path) {
        anyhow::bail!("CodeGraph not initialized");
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("{}", e))?;
    let queries = QueryBuilder::new(db.get_conn());

    // File mode
    if let Some(file_path) = file {
        let full_path = Path::new(path).join(file_path);
        let content = codegraph::util::read_file_content(&full_path)?;
        let lines: Vec<&str> = content.lines().collect();

        let start = offset.unwrap_or(1).saturating_sub(1);
        let end = limit.map(|l| (start + l).min(lines.len())).unwrap_or(lines.len());

        if symbols_only {
            // Show symbol map for file
            let options = codegraph::types::SearchOptions {
                file_pattern: Some(file_path.to_string()),
                ..Default::default()
            };
            let results = queries.search_nodes("", Some(&options))?;
            println!("Symbols in {}:", file_path);
            for result in &results {
                println!("  {} ({}):{}", result.node.name, result.node.kind.as_str(), result.node.start_line);
            }
        } else {
            println!("=== {} (lines {}-{}) ===", file_path, start + 1, end);
            for i in start..end {
                println!("{:4} | {}", i + 1, lines[i]);
            }
        }
        return Ok(());
    }

    // Symbol mode
    let options = codegraph::types::SearchOptions::default();
    let results = queries.search_nodes(name, Some(&options))?;

    if results.is_empty() {
        println!("No symbol found: {}", name);
        return Ok(());
    }

    let node = &results[0].node;
    println!("=== {} ({}) ===", node.name, node.kind.as_str());
    println!("  File: {}:{}", node.file_path, node.start_line);
    if let Some(ref sig) = node.signature {
        println!("  Signature: {}", sig);
    }

    // Read source
    let full_path = Path::new(path).join(&node.file_path);
    if let Ok(content) = codegraph::util::read_file_content(&full_path) {
        let lines: Vec<&str> = content.lines().collect();
        let start = (node.start_line as usize).saturating_sub(1);
        let end = (node.end_line as usize).min(lines.len());

        println!("\nSource:");
        for i in start..end {
            println!("{:4} | {}", i + 1, lines[i]);
        }
    }

    Ok(())
}

fn cmd_affected(
    path: &str,
    files: &[String],
    stdin: bool,
    depth: usize,
    filter: Option<&str>,
    json: bool,
    quiet: bool,
) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;
    use std::collections::{HashSet, VecDeque};
    use std::io::{self, BufRead};
    use globset::Glob;

    if !codegraph::db::is_initialized(path) {
        anyhow::bail!("CodeGraph not initialized");
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("{}", e))?;
    let queries = QueryBuilder::new(db.get_conn());

    // Collect files to analyze
    let mut changed_files: Vec<String> = files.to_vec();

    if stdin {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let line = line?;
            if !line.trim().is_empty() {
                changed_files.push(line.trim().to_string());
            }
        }
    }

    if changed_files.is_empty() {
        anyhow::bail!("No files specified. Provide files as arguments or use --stdin");
    }

    // Build test file matcher
    let default_test_patterns = [
        ".spec.", ".test.", "/__tests__/", "/tests?/", "/e2e/", "/spec/"
    ];
    let is_test_file = |file_path: &str| -> bool {
        if let Some(filter_glob) = filter {
            if let Ok(glob) = Glob::new(filter_glob) {
                if glob.compile_matcher().is_match(file_path) {
                    return true;
                }
            }
        }
        default_test_patterns.iter().any(|pat| file_path.contains(pat))
    };

    // BFS on file-level dependency graph
    let mut affected: HashSet<String> = HashSet::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();

    // Seed BFS with changed files
    for file in &changed_files {
        let rel = file.clone();
        if is_test_file(&rel) {
            affected.insert(rel.clone());
        }
        if visited.insert(rel.clone()) {
            queue.push_back((rel, 0));
        }
    }

    // BFS traversal
    while let Some((current_file, current_depth)) = queue.pop_front() {
        if current_depth >= depth {
            continue;
        }

        if let Ok(dependents) = queries.get_file_dependents(&current_file) {
            for dep_file in dependents {
                if !visited.insert(dep_file.clone()) {
                    continue;
                }
                if is_test_file(&dep_file) {
                    affected.insert(dep_file.clone());
                }
                queue.push_back((dep_file, current_depth + 1));
            }
        }
    }

    let mut affected_files: Vec<String> = affected.into_iter().collect();
    affected_files.sort();
    let total_affected = affected_files.len();
    let _affected_refs: Vec<&String> = affected_files.iter().collect();
    // For JSON output
    let _affected_slice = affected_files.clone();

    if json {
        let output = serde_json::json!({
            "changed": changed_files,
            "affected": affected_files,
            "count": total_affected,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if quiet {
        for file in &affected_files {
            println!("{}", file);
        }
    } else {
        println!("Changed files: {}", changed_files.len());
        println!("Affected files: {}", total_affected);
        for file in &affected_files {
            println!("  {}", file);
        }
    }

    Ok(())
}

fn cmd_install(
    target: Option<&str>,
    location: Option<&str>,
    yes: bool,
    no_permissions: bool,
    print_config: Option<&str>,
) -> anyhow::Result<()> {
    println!("Install command - Agent installation not yet implemented");
    println!("  Target: {:?}", target);
    println!("  Location: {:?}", location);
    println!("  Yes: {}", yes);
    println!("  No permissions: {}", no_permissions);
    println!("  Print config: {:?}", print_config);
    println!("\nFor now, manually add MCP server config to your AI agent.");
    Ok(())
}

fn cmd_uninstall(
    target: Option<&str>,
    location: Option<&str>,
    yes: bool,
) -> anyhow::Result<()> {
    println!("Uninstall command - Agent uninstallation not yet implemented");
    println!("  Target: {:?}", target);
    println!("  Location: {:?}", location);
    println!("  Yes: {}", yes);
    Ok(())
}

fn cmd_upgrade(
    version: Option<&str>,
    check: bool,
    _force: bool,
) -> anyhow::Result<()> {
    println!("CodeGraph v{}", env!("CARGO_PKG_VERSION"));

    if check {
        println!("Checking for updates...");
        println!("Auto-update not yet implemented for Rust version.");
        println!("Please rebuild from source to update.");
    } else if let Some(ver) = version {
        println!("Target version: {}", ver);
        println!("Auto-update not yet implemented for Rust version.");
    } else {
        println!("Current version: {}", env!("CARGO_PKG_VERSION"));
        println!("Auto-update not yet implemented for Rust version.");
        println!("Please rebuild from source: cargo build --release");
    }

    Ok(())
}
