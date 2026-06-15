use rusqlite::Connection;
use log::info;

const CURRENT_SCHEMA_VERSION: u32 = 5;

/// Initialize the database schema
pub fn initialize_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    info!("Initializing database schema");

    // Create schema versions table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_versions (
            version INTEGER PRIMARY KEY,
            applied_at INTEGER NOT NULL,
            description TEXT
        );"
    )?;

    // Check current version
    let current_version: Option<u32> = conn.query_row(
        "SELECT MAX(version) FROM schema_versions",
        [],
        |row| row.get(0)
    ).unwrap_or(None);

    if let Some(version) = current_version {
        if version >= CURRENT_SCHEMA_VERSION {
            info!("Schema already up to date (version {})", version);
            return Ok(());
        }
        info!("Current schema version: {}, upgrading to {}", version, CURRENT_SCHEMA_VERSION);
    } else {
        info!("No existing schema, creating fresh database");
    }

    // Run migrations
    run_migrations(conn, current_version.unwrap_or(0))?;

    Ok(())
}

/// Run migrations from current version to latest
fn run_migrations(conn: &Connection, from_version: u32) -> Result<(), rusqlite::Error> {
    let tx = conn.unchecked_transaction()?;

    for version in (from_version + 1)..=CURRENT_SCHEMA_VERSION {
        info!("Applying migration v{}", version);

        match version {
            1 => apply_migration_v1(&tx)?,
            2 => apply_migration_v2(&tx)?,
            3 => apply_migration_v3(&tx)?,
            4 => apply_migration_v4(&tx)?,
            5 => apply_migration_v5(&tx)?,
            _ => return Err(rusqlite::Error::ExecuteReturnedResults),
        }

        // Record migration
        let _: usize = tx.execute(
            "INSERT INTO schema_versions (version, applied_at, description) VALUES (?1, strftime('%s', 'now') * 1000, ?2)",
            [&version.to_string(), &format!("Migration to version {}", version)]
        )?;
    }

    tx.commit()?;
    info!("All migrations applied successfully");
    Ok(())
}

/// Migration v1: Initial schema
fn apply_migration_v1(conn: &Connection) -> Result<(), rusqlite::Error> {
    // Nodes table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS nodes (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            qualified_name TEXT NOT NULL,
            file_path TEXT NOT NULL,
            language TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            start_column INTEGER NOT NULL,
            end_column INTEGER NOT NULL,
            docstring TEXT,
            signature TEXT,
            visibility TEXT,
            is_exported INTEGER DEFAULT 0,
            is_async INTEGER DEFAULT 0,
            is_static INTEGER DEFAULT 0,
            is_abstract INTEGER DEFAULT 0,
            decorators TEXT,
            type_parameters TEXT,
            updated_at INTEGER NOT NULL
        );"
    )?;

    // Edges table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source TEXT NOT NULL,
            target TEXT NOT NULL,
            kind TEXT NOT NULL,
            metadata TEXT,
            line INTEGER,
            col INTEGER,
            provenance TEXT DEFAULT NULL,
            FOREIGN KEY (source) REFERENCES nodes(id) ON DELETE CASCADE,
            FOREIGN KEY (target) REFERENCES nodes(id) ON DELETE CASCADE
        );"
    )?;

    // Files table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS files (
            path TEXT PRIMARY KEY,
            content_hash TEXT NOT NULL,
            language TEXT NOT NULL,
            size INTEGER NOT NULL,
            modified_at INTEGER NOT NULL,
            indexed_at INTEGER NOT NULL,
            node_count INTEGER DEFAULT 0,
            errors TEXT
        );"
    )?;

    // Unresolved refs table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS unresolved_refs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            from_node_id TEXT NOT NULL,
            reference_name TEXT NOT NULL,
            reference_kind TEXT NOT NULL,
            line INTEGER NOT NULL,
            col INTEGER NOT NULL,
            candidates TEXT,
            file_path TEXT NOT NULL DEFAULT '',
            language TEXT NOT NULL DEFAULT 'unknown',
            FOREIGN KEY (from_node_id) REFERENCES nodes(id) ON DELETE CASCADE
        );"
    )?;

    // Create indexes
    create_indexes_v1(conn)?;

    // Create FTS5 virtual table and triggers
    create_fts5(conn)?;

    Ok(())
}

/// Create indexes for v1
fn create_indexes_v1(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch("
        -- Node indexes
        CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
        CREATE INDEX IF NOT EXISTS idx_nodes_name ON nodes(name);
        CREATE INDEX IF NOT EXISTS idx_nodes_qualified_name ON nodes(qualified_name);
        CREATE INDEX IF NOT EXISTS idx_nodes_file_path ON nodes(file_path);
        CREATE INDEX IF NOT EXISTS idx_nodes_language ON nodes(language);
        CREATE INDEX IF NOT EXISTS idx_nodes_file_line ON nodes(file_path, start_line);

        -- Edge indexes
        CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind);
        CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source);
        CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target);
        CREATE INDEX IF NOT EXISTS idx_edges_source_kind ON edges(source, kind);
        CREATE INDEX IF NOT EXISTS idx_edges_target_kind ON edges(target, kind);

        -- File indexes
        CREATE INDEX IF NOT EXISTS idx_files_language ON files(language);
        CREATE INDEX IF NOT EXISTS idx_files_modified_at ON files(modified_at);

        -- Unresolved refs indexes
        CREATE INDEX IF NOT EXISTS idx_unresolved_from_node ON unresolved_refs(from_node_id);
        CREATE INDEX IF NOT EXISTS idx_unresolved_name ON unresolved_refs(reference_name);
        CREATE INDEX IF NOT EXISTS idx_unresolved_file_path ON unresolved_refs(file_path);
        CREATE INDEX IF NOT EXISTS idx_unresolved_from_name ON unresolved_refs(from_node_id, reference_name);
    ")?;

    Ok(())
}

/// Create FTS5 full-text search
fn create_fts5(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch("
        -- FTS5 virtual table
        CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
            id,
            name,
            qualified_name,
            docstring,
            signature,
            content='nodes',
            content_rowid='rowid'
        );

        -- Triggers to keep FTS in sync
        CREATE TRIGGER IF NOT EXISTS nodes_ai AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, id, name, qualified_name, docstring, signature)
            VALUES (NEW.rowid, NEW.id, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_ad AFTER DELETE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, id, name, qualified_name, docstring, signature)
            VALUES ('delete', OLD.rowid, OLD.id, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature);
        END;

        CREATE TRIGGER IF NOT EXISTS nodes_au AFTER UPDATE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, id, name, qualified_name, docstring, signature)
            VALUES ('delete', OLD.rowid, OLD.id, OLD.name, OLD.qualified_name, OLD.docstring, OLD.signature);
            INSERT INTO nodes_fts(rowid, id, name, qualified_name, docstring, signature)
            VALUES (NEW.rowid, NEW.id, NEW.name, NEW.qualified_name, NEW.docstring, NEW.signature);
        END;
    ")?;

    Ok(())
}

/// Migration v2: Add project_metadata table
fn apply_migration_v2(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch("
        -- Project metadata table
        CREATE TABLE IF NOT EXISTS project_metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        );
    ")?;

    Ok(())
}

/// Migration v3: Add lower(name) expression index
fn apply_migration_v3(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch("
        CREATE INDEX IF NOT EXISTS idx_nodes_lower_name ON nodes(lower(name));
    ")?;

    Ok(())
}

/// Migration v4: Drop redundant edge indexes (covered by composites)
fn apply_migration_v4(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch("
        DROP INDEX IF EXISTS idx_edges_source;
        DROP INDEX IF EXISTS idx_edges_target;
    ")?;

    Ok(())
}

/// Migration v5: Add return_type column for C++ receiver-type inference
fn apply_migration_v5(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch("
        ALTER TABLE nodes ADD COLUMN return_type TEXT;
    ")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::db::connection::DatabaseConnection;

    #[test]
    fn test_initialize_schema() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = DatabaseConnection::initialize(db_path.to_str().unwrap()).unwrap();

        initialize_schema(db.get_conn()).unwrap();

        // Verify tables exist
        let tables: Vec<String> = db.get_conn().prepare(
            "SELECT name FROM sqlite_master WHERE type='table'"
        ).unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert!(tables.contains(&"nodes".to_string()));
        assert!(tables.contains(&"edges".to_string()));
        assert!(tables.contains(&"files".to_string()));
        assert!(tables.contains(&"unresolved_refs".to_string()));
        assert!(tables.contains(&"schema_versions".to_string()));
    }

    #[test]
    fn test_schema_version() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = DatabaseConnection::initialize(db_path.to_str().unwrap()).unwrap();

        initialize_schema(db.get_conn()).unwrap();

        let version: u32 = db.get_conn().query_row(
            "SELECT MAX(version) FROM schema_versions",
            [],
            |row| row.get(0)
        ).unwrap();

        assert_eq!(version, CURRENT_SCHEMA_VERSION);
    }
}
