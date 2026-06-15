use rusqlite::Connection;
use std::path::Path;
use log::{info, debug};

/// SQLite backend type
#[derive(Debug, Clone, PartialEq)]
pub enum SqliteBackend {
    NodeSqlite,  // For compatibility with TS version
    Rusqlite,
}

/// Database connection wrapper
pub struct DatabaseConnection {
    conn: Connection,
    db_path: String,
}

impl DatabaseConnection {
    /// Initialize a new database at the given path
    pub fn initialize(db_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Initializing database at {}", db_path);

        // Create parent directory if needed
        if let Some(parent) = Path::new(db_path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        info!("Opening connection to {}", db_path);
        let conn = Connection::open(db_path)?;
        info!("Connection opened successfully");

        let db = Self {
            conn,
            db_path: db_path.to_string(),
        };

        info!("Configuring connection");
        db.configure_connection()?;
        info!("Configuration complete");
        Ok(db)
    }

    /// Open an existing database
    pub fn open(db_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Opening database at {}", db_path);

        if !Path::new(db_path).exists() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Database file not found: {}", db_path)
            )));
        }

        let conn = Connection::open(db_path)?;
        let db = Self {
            conn,
            db_path: db_path.to_string(),
        };

        db.configure_connection()?;
        Ok(db)
    }

    /// Configure connection with optimal settings
    fn configure_connection(&self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Configuring database connection");

        // Helper function to execute PRAGMA without checking results
        let pragma = |sql: &str| -> Result<(), rusqlite::Error> {
            let mut stmt = self.conn.prepare(sql)?;
            let _rows = stmt.query([])?;
            Ok(())
        };

        pragma("PRAGMA busy_timeout = 5000")?;
        pragma("PRAGMA foreign_keys = ON")?;
        
        // WAL mode returns the journal mode
        let mode: String = self.conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
        debug!("Journal mode set to: {}", mode);
        
        pragma("PRAGMA synchronous = NORMAL")?;
        pragma("PRAGMA cache_size = -64000")?;
        pragma("PRAGMA temp_store = MEMORY")?;
        pragma("PRAGMA mmap_size = 268435456")?;

        debug!("Database configuration complete");
        Ok(())
    }

    /// Get the underlying connection (for queries)
    pub fn get_conn(&self) -> &Connection {
        &self.conn
    }

    /// Get the database path
    pub fn get_path(&self) -> &str {
        &self.db_path
    }

    /// Get the SQLite backend type
    pub fn get_backend(&self) -> SqliteBackend {
        SqliteBackend::Rusqlite
    }

    /// Get the current journal mode
    pub fn get_journal_mode(&self) -> Result<String, rusqlite::Error> {
        let mode: String = self.conn.query_row(
            "PRAGMA journal_mode",
            [],
            |row| row.get(0)
        )?;
        Ok(mode)
    }

    /// Get database size in bytes
    pub fn get_size(&self) -> u64 {
        match std::fs::metadata(&self.db_path) {
            Ok(meta) => meta.len(),
            Err(_) => 0,
        }
    }

    /// Run maintenance operations (ANALYZE, checkpoint WAL)
    pub fn run_maintenance(&self) -> Result<(), rusqlite::Error> {
        debug!("Running database maintenance");

        // Checkpoint WAL
        self.conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE)")?;

        // Update statistics for query planner
        self.conn.execute_batch("ANALYZE")?;

        Ok(())
    }

    /// Optimize the database (VACUUM)
    pub fn optimize(&self) -> Result<(), rusqlite::Error> {
        info!("Optimizing database");
        self.conn.execute_batch("VACUUM")?;
        self.conn.execute_batch("ANALYZE")?;
        Ok(())
    }

    /// Close the database connection
    pub fn close(self) {
        drop(self);
    }
}

// Helper function to get database path from project root
pub fn get_database_path(project_root: &str) -> String {
    let codegraph_dir = get_codegraph_dir(project_root);
    format!("{}/codegraph.db", codegraph_dir)
}

/// Get the .codegraph directory path
pub fn get_codegraph_dir(project_root: &str) -> String {
    // Allow override via environment variable
    if let Ok(dir) = std::env::var("CODEGRAPH_DIR") {
        return format!("{}/{}", project_root, dir);
    }
    format!("{}/.codegraph", project_root)
}

/// Check if a project is initialized
pub fn is_initialized(project_root: &str) -> bool {
    let codegraph_dir = get_codegraph_dir(project_root);
    let db_path = format!("{}/codegraph.db", codegraph_dir);
    Path::new(&db_path).exists()
}

/// Create the .codegraph directory structure
pub fn create_directory(project_root: &str) -> Result<(), std::io::Error> {
    let codegraph_dir = get_codegraph_dir(project_root);
    std::fs::create_dir_all(&codegraph_dir)?;
    info!("Created CodeGraph directory at {}", codegraph_dir);
    Ok(())
}

/// Remove the .codegraph directory
pub fn remove_directory(project_root: &str) -> Result<(), std::io::Error> {
    let codegraph_dir = get_codegraph_dir(project_root);
    std::fs::remove_dir_all(&codegraph_dir)?;
    info!("Removed CodeGraph directory at {}", codegraph_dir);
    Ok(())
}

/// Validate directory structure
pub fn validate_directory(project_root: &str) -> ValidationResult {
    let codegraph_dir = get_codegraph_dir(project_root);
    let db_path = format!("{}/codegraph.db", codegraph_dir);

    let mut errors = Vec::new();

    if !Path::new(&codegraph_dir).exists() {
        errors.push("CodeGraph directory not found".to_string());
    }

    if !Path::new(&db_path).exists() {
        errors.push("Database file not found".to_string());
    }

    ValidationResult {
        valid: errors.is_empty(),
        errors,
    }
}

pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}
