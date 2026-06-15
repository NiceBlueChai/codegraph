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
