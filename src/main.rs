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
    },

    /// Index the project
    Index {
        /// Project root directory
        #[arg(default_value = ".")]
        path: String,
    },

    /// Sync changes
    Sync {
        /// Project root directory
        #[arg(default_value = ".")]
        path: String,
    },

    /// Show project status
    Status {
        /// Project root directory
        #[arg(default_value = ".")]
        path: String,
    },

    /// Query symbols
    Query {
        /// Search query
        query: String,

        /// Project root directory
        #[arg(default_value = ".")]
        path: String,
    },

    /// Start MCP server
    Serve {
        /// Run in MCP mode
        #[arg(long)]
        mcp: bool,
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
    },

    /// Explore symbol area: source + call paths
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
        Some(Commands::Init { path }) => {
            info!("Initializing CodeGraph project at {}", path);
            cmd_init(&path)?;
        }
        Some(Commands::Index { path }) => {
            info!("Indexing project at {}", path);
            cmd_index(&path)?;
        }
        Some(Commands::Sync { path }) => {
            info!("Syncing changes at {}", path);
            cmd_sync(&path)?;
        }
        Some(Commands::Status { path }) => {
            info!("Checking status at {}", path);
            cmd_status(&path)?;
        }
        Some(Commands::Query { query, path }) => {
            info!("Querying at {}: {}", path, query);
            cmd_query(&path, &query)?;
        }
        Some(Commands::Serve { mcp }) => {
            info!("Starting MCP server (mcp={})", mcp);
            cmd_serve(mcp)?;
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
        Some(Commands::Files { path, filter, pattern, format, max_depth, json }) => {
            info!("Showing files at {}", path);
            cmd_files(&path, filter.as_deref(), pattern.as_deref(), &format, max_depth, json)?;
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

fn cmd_init(path: &str) -> anyhow::Result<()> {
    use codegraph::db::{get_database_path, create_directory};

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

    Ok(())
}

fn cmd_index(path: &str) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;
    use codegraph::core::indexer::Indexer;

    if !codegraph::db::is_initialized(path) {
        anyhow::bail!("CodeGraph not initialized. Run 'codegraph init' first.");
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("Failed to open database: {}", e))?;
    initialize_schema(db.get_conn())?;

    let queries = QueryBuilder::new(db.get_conn());
    let indexer = Indexer::new(queries, path);

    println!("Indexing...");
    let result = indexer.index_all().map_err(|e| anyhow::anyhow!("Indexing failed: {}", e))?;
    println!("✓ Indexing complete");
    println!("  Files indexed: {}", result.files_indexed);
    println!("  Files skipped: {}", result.files_skipped);
    println!("  Nodes created: {}", result.nodes_created);
    println!("  Edges created: {}", result.edges_created);

    Ok(())
}

fn cmd_sync(path: &str) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;
    use codegraph::core::indexer::Indexer;

    if !codegraph::db::is_initialized(path) {
        anyhow::bail!("CodeGraph not initialized. Run 'codegraph init' first.");
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("Failed to open database: {}", e))?;

    println!("Syncing...");
    let indexer = Indexer::new(QueryBuilder::new(db.get_conn()), path);
    let result = indexer.sync().map_err(|e| anyhow::anyhow!("Sync failed: {}", e))?;

    println!("✓ Sync complete");
    println!("  Files checked: {}", result.files_checked);
    println!("  Files added: {}", result.files_added);
    println!("  Files modified: {}", result.files_modified);
    println!("  Duration: {}ms", result.duration_ms);

    Ok(())
}

fn cmd_status(path: &str) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;

    if !codegraph::db::is_initialized(path) {
        println!("CodeGraph not initialized");
        return Ok(());
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("Failed to open database: {}", e))?;
    let queries = QueryBuilder::new(db.get_conn());
    let stats = queries.get_stats()?;

    println!("CodeGraph Status:");
    println!("  Backend: {:?}", db.get_backend());
    println!("  Journal Mode: {}", db.get_journal_mode().unwrap_or_else(|_| "unknown".to_string()));
    println!("  Nodes: {}", stats.node_count);
    println!("  Edges: {}", stats.edge_count);
    println!("  Files: {}", stats.file_count);
    println!("  Unresolved Refs: {}", stats.unresolved_ref_count);

    Ok(())
}

fn cmd_query(path: &str, query: &str) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;

    if !codegraph::db::is_initialized(path) {
        anyhow::bail!("CodeGraph not initialized");
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("Failed to open database: {}", e))?;
    let queries = QueryBuilder::new(db.get_conn());

    let options = codegraph::types::SearchOptions::default();
    let results = queries.search_nodes(query, Some(&options))?;

    if results.is_empty() {
        println!("No results found for '{}'", query);
    } else {
        println!("Found {} results for '{}':", results.len(), query);
        for result in &results {
            println!("  - {} ({}) in {}", result.node.name, result.node.kind.as_str(), result.node.file_path);
            if let Some(ref sig) = result.node.signature {
                println!("    Signature: {}", sig);
            }
        }
    }

    Ok(())
}

fn cmd_serve(mcp: bool) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;

    if mcp {
        // Get project root (current directory by default)
        let project_root = ".";

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
    }

    Ok(())
}

fn cmd_callers(path: &str, symbol: &str, limit: usize, json: bool) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;
    use codegraph::core::query::GraphTraverser;

    if !codegraph::db::is_initialized(path) {
        anyhow::bail!("CodeGraph not initialized");
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("{}", e))?;
    let queries = QueryBuilder::new(db.get_conn());
    let traverser = GraphTraverser { queries };

    let callers = traverser.get_callers(symbol, 1).map_err(|e| anyhow::anyhow!("{}", e))?;

    if json {
        let output = serde_json::json!({
            "symbol": symbol,
            "callers": callers.iter().take(limit).map(|(node, edge)| {
                serde_json::json!({
                    "name": node.name,
                    "file": node.file_path,
                    "line": node.start_line,
                    "kind": node.kind.as_str(),
                    "edge_kind": edge.kind.as_str(),
                })
            }).collect::<Vec<_>>(),
            "total": callers.len(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        if callers.is_empty() {
            println!("No callers found for '{}'", symbol);
        } else {
            println!("Callers of '{}' ({}):", symbol, callers.len());
            for (node, edge) in callers.iter().take(limit) {
                println!("  - {} ({}) at {}:{}", node.name, node.kind.as_str(), node.file_path, node.start_line);
            }
        }
    }

    Ok(())
}

fn cmd_callees(path: &str, symbol: &str, limit: usize, json: bool) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;
    use codegraph::core::query::GraphTraverser;

    if !codegraph::db::is_initialized(path) {
        anyhow::bail!("CodeGraph not initialized");
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("{}", e))?;
    let queries = QueryBuilder::new(db.get_conn());
    let traverser = GraphTraverser { queries };

    let callees = traverser.get_callees(symbol, 1).map_err(|e| anyhow::anyhow!("{}", e))?;

    if json {
        let output = serde_json::json!({
            "symbol": symbol,
            "callees": callees.iter().take(limit).map(|(node, edge)| {
                serde_json::json!({
                    "name": node.name,
                    "file": node.file_path,
                    "line": node.start_line,
                    "kind": node.kind.as_str(),
                    "edge_kind": edge.kind.as_str(),
                })
            }).collect::<Vec<_>>(),
            "total": callees.len(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        if callees.is_empty() {
            println!("No callees found for '{}'", symbol);
        } else {
            println!("Callees of '{}' ({}):", symbol, callees.len());
            for (node, edge) in callees.iter().take(limit) {
                println!("  - {} ({}) at {}:{}", node.name, node.kind.as_str(), node.file_path, node.start_line);
            }
        }
    }

    Ok(())
}

fn cmd_impact(path: &str, symbol: &str, depth: usize, json: bool) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;
    use codegraph::core::query::GraphTraverser;

    if !codegraph::db::is_initialized(path) {
        anyhow::bail!("CodeGraph not initialized");
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("{}", e))?;
    let queries = QueryBuilder::new(db.get_conn());
    let traverser = GraphTraverser { queries };

    let impact = traverser.get_impact_radius(symbol, depth).map_err(|e| anyhow::anyhow!("{}", e))?;

    if json {
        let output = serde_json::json!({
            "symbol": symbol,
            "depth": depth,
            "affected": impact.nodes.values().map(|node| {
                serde_json::json!({
                    "name": node.name,
                    "file": node.file_path,
                    "line": node.start_line,
                    "kind": node.kind.as_str(),
                })
            }).collect::<Vec<_>>(),
            "total": impact.nodes.len(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        if impact.nodes.is_empty() {
            println!("No impact found for '{}'", symbol);
        } else {
            println!("Impact analysis for '{}' (depth {}):", symbol, depth);
            println!("  Affected symbols: {}", impact.nodes.len());
            for node in impact.nodes.values() {
                println!("  - {} ({}) at {}:{}", node.name, node.kind.as_str(), node.file_path, node.start_line);
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
        println!("This will remove all CodeGraph data in {}", path);
        println!("Use --force to skip this confirmation");
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
) -> anyhow::Result<()> {
    use std::path::Path;
    use ignore::WalkBuilder;
    use globset::Glob;

    let root = Path::new(path);
    if !root.exists() {
        anyhow::bail!("Path does not exist: {}", path);
    }

    let mut walker = WalkBuilder::new(root);
    walker.hidden(false);

    if let Some(depth) = max_depth {
        walker.max_depth(Some(depth));
    }

    let glob_matcher = if let Some(pat) = pattern {
        Some(Glob::new(pat)?.compile_matcher())
    } else {
        None
    };

    let filter_dir = filter.map(|f| root.join(f));

    let mut files: Vec<String> = Vec::new();

    for entry in walker.build() {
        let entry = entry?;
        let entry_path = entry.path();

        // Apply directory filter
        if let Some(ref filter_path) = filter_dir {
            if !entry_path.starts_with(filter_path) {
                continue;
            }
        }

        // Apply glob pattern
        if let Some(ref glob) = glob_matcher {
            if !glob.is_match(entry_path.file_name().unwrap_or_default()) {
                continue;
            }
        }

        if entry_path.is_file() {
            if let Ok(rel) = entry_path.strip_prefix(root) {
                files.push(rel.to_string_lossy().to_string());
            }
        }
    }

    if json {
        let output = serde_json::json!({
            "root": path,
            "files": files,
            "count": files.len(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        match format {
            "flat" => {
                for file in &files {
                    println!("{}", file);
                }
            }
            "grouped" => {
                let mut by_ext: std::collections::BTreeMap<String, Vec<&String>> = std::collections::BTreeMap::new();
                for file in &files {
                    let ext = Path::new(file)
                        .extension()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    by_ext.entry(ext).or_default().push(file);
                }
                for (ext, ext_files) in &by_ext {
                    println!(".{} ({} files):", ext, ext_files.len());
                    for file in ext_files {
                        println!("  {}", file);
                    }
                }
            }
            _ => {
                // tree format (default)
                println!("{}/", path);
                for file in &files {
                    let depth = file.matches('/').count();
                    let indent = "  ".repeat(depth);
                    let name = Path::new(file).file_name().unwrap_or_default().to_string_lossy();
                    println!("{}{}", indent, name);
                }
            }
        }
        println!("\n{} files", files.len());
    }

    Ok(())
}

fn cmd_explore(path: &str, query: &[String], max_files: usize) -> anyhow::Result<()> {
    use codegraph::db::get_database_path;
    use std::fs;

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

    let mut files_shown = 0;

    for result in results.iter() {
        if files_shown >= max_files {
            break;
        }

        let node = &result.node;
        println!("=== {} ({}) ===", node.name, node.kind.as_str());
        println!("  File: {}:{}", node.file_path, node.start_line);

        // Try to read source
        let full_path = std::path::Path::new(path).join(&node.file_path);
        if let Ok(content) = fs::read_to_string(&full_path) {
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
    use std::fs;
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
        let content = fs::read_to_string(&full_path)?;
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
    if let Ok(content) = fs::read_to_string(&full_path) {
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
    use codegraph::core::query::GraphTraverser;
    use std::io::{self, BufRead};

    if !codegraph::db::is_initialized(path) {
        anyhow::bail!("CodeGraph not initialized");
    }

    let db_path = get_database_path(path);
    let db = DatabaseConnection::open(&db_path).map_err(|e| anyhow::anyhow!("{}", e))?;
    let queries = QueryBuilder::new(db.get_conn());
    let traverser_queries = QueryBuilder::new(db.get_conn());
    let traverser = GraphTraverser { queries: traverser_queries };

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

    // Find symbols in changed files and their impact
    let mut affected: std::collections::HashSet<String> = std::collections::HashSet::new();

    for file in &changed_files {
        let options = codegraph::types::SearchOptions {
            file_pattern: Some(file.clone()),
            ..Default::default()
        };
        let results = queries.search_nodes("", Some(&options))?;

        for result in &results {
            let impact = traverser.get_impact_radius(&result.node.name, depth).map_err(|e| anyhow::anyhow!("{}", e))?;
            for node in impact.nodes.values() {
                // Apply test file filter
                if let Some(filter_glob) = filter {
                    let glob = globset::Glob::new(filter_glob)?.compile_matcher();
                    if !glob.is_match(&node.file_path) {
                        continue;
                    }
                }
                affected.insert(node.file_path.clone());
            }
        }
    }

    let affected_files: Vec<&String> = affected.iter().collect();

    if json {
        let output = serde_json::json!({
            "changed": changed_files,
            "affected": affected_files,
            "count": affected_files.len(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if quiet {
        for file in &affected_files {
            println!("{}", file);
        }
    } else {
        println!("Changed files: {}", changed_files.len());
        println!("Affected files: {}", affected_files.len());
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
    force: bool,
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
