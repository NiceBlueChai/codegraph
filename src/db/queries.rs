use rusqlite::{Connection, Transaction, params, OptionalExtension};
use std::collections::HashMap;
use std::str::FromStr;
use log::debug;
use crate::types::*;

/// Query builder with prepared statements
pub struct QueryBuilder<'a> {
    conn: &'a Connection,
}

impl<'a> QueryBuilder<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn get_conn(&self) -> &Connection {
        self.conn
    }

    // ========================================================================
    // Node Operations
    // ========================================================================

    /// Insert a single node
    pub fn insert_node(&self, node: &Node) -> Result<(), rusqlite::Error> {
        let tx = self.conn.unchecked_transaction()?;
        self.insert_node_tx(&tx, node)?;
        tx.commit()?;
        Ok(())
    }

    /// Insert a node within a transaction
    pub fn insert_node_tx(&self, tx: &Transaction, node: &Node) -> Result<(), rusqlite::Error> {
        tx.execute(
            "INSERT OR REPLACE INTO nodes (
                id, kind, name, qualified_name, file_path, language,
                start_line, end_line, start_column, end_column,
                docstring, signature, visibility, is_exported,
                is_async, is_static, is_abstract, decorators,
                type_parameters, return_type, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                     ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                node.id,
                node.kind.as_str(),
                node.name,
                node.qualified_name,
                node.file_path,
                node.language.as_str(),
                node.start_line,
                node.end_line,
                node.start_column,
                node.end_column,
                node.docstring,
                node.signature,
                node.visibility,
                if node.is_exported { 1 } else { 0 },
                if node.is_async { 1 } else { 0 },
                if node.is_static { 1 } else { 0 },
                if node.is_abstract { 1 } else { 0 },
                serde_json::to_string(&node.decorators).ok(),
                serde_json::to_string(&node.type_parameters).ok(),
                node.return_type,
                node.updated_at,
            ])?;
        Ok(())
    }

    /// Batch insert nodes (optimized with transaction)
    pub fn insert_nodes_batch(&self, nodes: &[Node]) -> Result<usize, rusqlite::Error> {
        if nodes.is_empty() {
            return Ok(0);
        }

        let tx = self.conn.unchecked_transaction()?;
        let mut count = 0;

        for node in nodes {
            self.insert_node_tx(&tx, node)?;
            count += 1;
        }

        tx.commit()?;
        debug!("Inserted {} nodes", count);
        Ok(count)
    }

    /// Get a node by ID
    pub fn get_node_by_id(&self, id: &str) -> Result<Option<Node>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, qualified_name, file_path, language,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_exported,
                    is_async, is_static, is_abstract, decorators,
                    type_parameters, return_type, updated_at
             FROM nodes WHERE id = ?1"
        )?;

        let node = stmt.query_row(params![id], |row| {
            Ok(Node {
                id: row.get(0)?,
                kind: NodeKind::from_str(&row.get::<_, String>(1)?).unwrap(),
                name: row.get(2)?,
                qualified_name: row.get(3)?,
                file_path: row.get(4)?,
                language: Language::from_str(&row.get::<_, String>(5)?).unwrap(),
                start_line: row.get(6)?,
                end_line: row.get(7)?,
                start_column: row.get(8)?,
                end_column: row.get(9)?,
                docstring: row.get(10)?,
                signature: row.get(11)?,
                visibility: row.get(12)?,
                is_exported: row.get::<_, i32>(13)? == 1,
                is_async: row.get::<_, i32>(14)? == 1,
                is_static: row.get::<_, i32>(15)? == 1,
                is_abstract: row.get::<_, i32>(16)? == 1,
                decorators: serde_json::from_str(&row.get::<_, String>(17)?).unwrap_or_default(),
                type_parameters: serde_json::from_str(&row.get::<_, String>(18)?).unwrap_or_default(),
                return_type: row.get(19)?,
                updated_at: row.get(20)?,
            })
        }).optional()?;

        Ok(node)
    }

    /// Get all nodes in a file
    pub fn get_nodes_by_file(&self, file_path: &str) -> Result<Vec<Node>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, qualified_name, file_path, language,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_exported,
                    is_async, is_static, is_abstract, decorators,
                    type_parameters, return_type, updated_at
             FROM nodes WHERE file_path = ?1 ORDER BY start_line"
        )?;

        let nodes = stmt.query_map(params![file_path], |row| {
            Ok(Node {
                id: row.get(0)?,
                kind: NodeKind::from_str(&row.get::<_, String>(1)?).unwrap(),
                name: row.get(2)?,
                qualified_name: row.get(3)?,
                file_path: row.get(4)?,
                language: Language::from_str(&row.get::<_, String>(5)?).unwrap(),
                start_line: row.get(6)?,
                end_line: row.get(7)?,
                start_column: row.get(8)?,
                end_column: row.get(9)?,
                docstring: row.get(10)?,
                signature: row.get(11)?,
                visibility: row.get(12)?,
                is_exported: row.get::<_, i32>(13)? == 1,
                is_async: row.get::<_, i32>(14)? == 1,
                is_static: row.get::<_, i32>(15)? == 1,
                is_abstract: row.get::<_, i32>(16)? == 1,
                decorators: serde_json::from_str(&row.get::<_, String>(17)?).unwrap_or_default(),
                type_parameters: serde_json::from_str(&row.get::<_, String>(18)?).unwrap_or_default(),
                return_type: row.get(19)?,
                updated_at: row.get(20)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(nodes)
    }

    /// Get nodes by kind
    pub fn get_nodes_by_kind(&self, kind: &NodeKind) -> Result<Vec<Node>, rusqlite::Error> {
        let kind_str = kind.as_str();
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, qualified_name, file_path, language,
                    start_line, end_line, start_column, end_column,
                    docstring, signature, visibility, is_exported,
                    is_async, is_static, is_abstract, decorators,
                    type_parameters, return_type, updated_at
             FROM nodes WHERE kind = ?1"
        )?;

        let nodes = stmt.query_map(params![kind_str], |row| {
            Ok(Node {
                id: row.get(0)?,
                kind: NodeKind::from_str(&row.get::<_, String>(1)?).unwrap(),
                name: row.get(2)?,
                qualified_name: row.get(3)?,
                file_path: row.get(4)?,
                language: Language::from_str(&row.get::<_, String>(5)?).unwrap(),
                start_line: row.get(6)?,
                end_line: row.get(7)?,
                start_column: row.get(8)?,
                end_column: row.get(9)?,
                docstring: row.get(10)?,
                signature: row.get(11)?,
                visibility: row.get(12)?,
                is_exported: row.get::<_, i32>(13)? == 1,
                is_async: row.get::<_, i32>(14)? == 1,
                is_static: row.get::<_, i32>(15)? == 1,
                is_abstract: row.get::<_, i32>(16)? == 1,
                decorators: serde_json::from_str(&row.get::<_, String>(17)?).unwrap_or_default(),
                type_parameters: serde_json::from_str(&row.get::<_, String>(18)?).unwrap_or_default(),
                return_type: row.get(19)?,
                updated_at: row.get(20)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(nodes)
    }

    /// Get multiple nodes by their IDs (batch query)
    pub fn get_nodes_by_ids(&self, ids: &[String]) -> Result<HashMap<String, Node>, rusqlite::Error> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut result = HashMap::new();
        
        // Query each ID individually to avoid complex parameter binding
        for id in ids {
            if let Some(node) = self.get_node_by_id(id)? {
                result.insert(id.clone(), node);
            }
        }

        Ok(result)
    }

    /// Delete nodes for a file (before re-indexing)
    pub fn delete_nodes_for_file(&self, file_path: &str) -> Result<usize, rusqlite::Error> {
        let tx = self.conn.unchecked_transaction()?;

        let count = tx.execute(
            "DELETE FROM nodes WHERE file_path = ?1",
            params![file_path]
        )?;

        tx.commit()?;
        Ok(count)
    }

    // ========================================================================
    // Edge Operations
    // ========================================================================

    /// Insert a single edge
    pub fn insert_edge(&self, edge: &Edge) -> Result<(), rusqlite::Error> {
        let tx = self.conn.unchecked_transaction()?;
        self.insert_edge_tx(&tx, edge)?;
        tx.commit()?;
        Ok(())
    }

    /// Insert an edge within a transaction
    pub fn insert_edge_tx(&self, tx: &Transaction, edge: &Edge) -> Result<(), rusqlite::Error> {
        tx.execute(
            "INSERT INTO edges (source, target, kind, metadata, line, col, provenance)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                edge.source,
                edge.target,
                edge.kind.as_str(),
                edge.metadata.as_ref().map(|m| serde_json::to_string(m).ok()).flatten(),
                edge.line,
                edge.column,
                edge.provenance.as_ref().map(|p| p.as_str()),
            ])?;
        Ok(())
    }

    /// Batch insert edges (optimized with transaction)
    pub fn insert_edges_batch(&self, edges: &[Edge]) -> Result<usize, rusqlite::Error> {
        if edges.is_empty() {
            return Ok(0);
        }

        let tx = self.conn.unchecked_transaction()?;
        let mut count = 0;

        for edge in edges {
            self.insert_edge_tx(&tx, edge)?;
            count += 1;
        }

        tx.commit()?;
        debug!("Inserted {} edges", count);
        Ok(count)
    }

    /// Get outgoing edges from a node
    pub fn get_outgoing_edges(&self, node_id: &str) -> Result<Vec<Edge>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT source, target, kind, metadata, line, col, provenance
             FROM edges WHERE source = ?1"
        )?;

        let edges = stmt.query_map(params![node_id], |row| {
            Ok(Edge {
                source: row.get(0)?,
                target: row.get(1)?,
                kind: EdgeKind::from_str(&row.get::<_, String>(2)?).unwrap(),
                metadata: row.get::<_, Option<String>>(3)?
                    .and_then(|s| serde_json::from_str(&s).ok()),
                line: row.get(4)?,
                column: row.get(5)?,
                provenance: row.get::<_, Option<String>>(6)?
                    .and_then(|s| match s.as_str() {
                        "tree-sitter" => Some(Provenance::TreeSitter),
                        "scip" => Some(Provenance::Scip),
                        "heuristic" => Some(Provenance::Heuristic),
                        _ => None,
                    }),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(edges)
    }

    /// Get incoming edges to a node
    pub fn get_incoming_edges(&self, node_id: &str) -> Result<Vec<Edge>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT source, target, kind, metadata, line, col, provenance
             FROM edges WHERE target = ?1"
        )?;

        let edges = stmt.query_map(params![node_id], |row| {
            Ok(Edge {
                source: row.get(0)?,
                target: row.get(1)?,
                kind: EdgeKind::from_str(&row.get::<_, String>(2)?).unwrap(),
                metadata: row.get::<_, Option<String>>(3)?
                    .and_then(|s| serde_json::from_str(&s).ok()),
                line: row.get(4)?,
                column: row.get(5)?,
                provenance: row.get::<_, Option<String>>(6)?
                    .and_then(|s| match s.as_str() {
                        "tree-sitter" => Some(Provenance::TreeSitter),
                        "scip" => Some(Provenance::Scip),
                        "heuristic" => Some(Provenance::Heuristic),
                        _ => None,
                    }),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(edges)
    }

    /// Get edges by kind
    pub fn get_edges_by_kind(&self, kind: &EdgeKind) -> Result<Vec<Edge>, rusqlite::Error> {
        let kind_str = kind.as_str();
        let mut stmt = self.conn.prepare(
            "SELECT source, target, kind, metadata, line, col, provenance
             FROM edges WHERE kind = ?1"
        )?;

        let edges = stmt.query_map(params![kind_str], |row| {
            Ok(Edge {
                source: row.get(0)?,
                target: row.get(1)?,
                kind: EdgeKind::from_str(&row.get::<_, String>(2)?).unwrap(),
                metadata: row.get::<_, Option<String>>(3)?
                    .and_then(|s| serde_json::from_str(&s).ok()),
                line: row.get(4)?,
                column: row.get(5)?,
                provenance: row.get::<_, Option<String>>(6)?
                    .and_then(|s| match s.as_str() {
                        "tree-sitter" => Some(Provenance::TreeSitter),
                        "scip" => Some(Provenance::Scip),
                        "heuristic" => Some(Provenance::Heuristic),
                        _ => None,
                    }),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(edges)
    }

    /// Delete edges for a node
    pub fn delete_edges_for_node(&self, node_id: &str) -> Result<usize, rusqlite::Error> {
        let count = self.conn.execute(
            "DELETE FROM edges WHERE source = ?1 OR target = ?1",
            params![node_id]
        )?;
        Ok(count)
    }

    // ========================================================================
    // File Operations
    // ========================================================================

    /// Insert or update a file record
    pub fn upsert_file(&self, file: &FileRecord) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT OR REPLACE INTO files (path, content_hash, language, size, modified_at, indexed_at, node_count, errors)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                file.path,
                file.content_hash,
                file.language.as_str(),
                file.size,
                file.modified_at,
                file.indexed_at,
                file.node_count,
                serde_json::to_string(&file.errors).ok(),
            ]
        )?;
        Ok(())
    }

    /// Get a file by path
    pub fn get_file_by_path(&self, path: &str) -> Result<Option<FileRecord>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT path, content_hash, language, size, modified_at, indexed_at, node_count, errors
             FROM files WHERE path = ?1"
        )?;

        let file = stmt.query_row(params![path], |row| {
            Ok(FileRecord {
                path: row.get(0)?,
                content_hash: row.get(1)?,
                language: Language::from_str(&row.get::<_, String>(2)?).unwrap(),
                size: row.get(3)?,
                modified_at: row.get(4)?,
                indexed_at: row.get(5)?,
                node_count: row.get(6)?,
                errors: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
            })
        }).optional()?;

        Ok(file)
    }

    /// Get all files
    pub fn get_all_files(&self) -> Result<Vec<FileRecord>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT path, content_hash, language, size, modified_at, indexed_at, node_count, errors
             FROM files"
        )?;

        let files = stmt.query_map([], |row| {
            Ok(FileRecord {
                path: row.get(0)?,
                content_hash: row.get(1)?,
                language: Language::from_str(&row.get::<_, String>(2)?).unwrap(),
                size: row.get(3)?,
                modified_at: row.get(4)?,
                indexed_at: row.get(5)?,
                node_count: row.get(6)?,
                errors: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(files)
    }

    /// Delete a file record
    pub fn delete_file(&self, path: &str) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "DELETE FROM files WHERE path = ?1",
            params![path]
        )?;
        Ok(())
    }

    // ========================================================================
    // Unresolved References
    // ========================================================================

    /// Insert an unresolved reference
    pub fn insert_unresolved_ref(&self, reference: &UnresolvedReference) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO unresolved_refs (from_node_id, reference_name, reference_kind, line, col, candidates, file_path, language)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                reference.from_node_id,
                reference.reference_name,
                reference.reference_kind,
                reference.line,
                reference.col,
                reference.candidates.as_ref().map(|c| serde_json::to_string(c).ok()).flatten(),
                reference.file_path,
                reference.language,
            ]
        )?;
        Ok(())
    }

    /// Get all unresolved references
    pub fn get_unresolved_references(&self) -> Result<Vec<UnresolvedReference>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, from_node_id, reference_name, reference_kind, line, col, candidates, file_path, language
             FROM unresolved_refs"
        )?;

        let refs = stmt.query_map([], |row| {
            Ok(UnresolvedReference {
                id: row.get(0)?,
                from_node_id: row.get(1)?,
                reference_name: row.get(2)?,
                reference_kind: row.get(3)?,
                line: row.get(4)?,
                col: row.get(5)?,
                candidates: row.get::<_, Option<String>>(6)?
                    .and_then(|s| serde_json::from_str(&s).ok()),
                file_path: row.get(7)?,
                language: row.get(8)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(refs)
    }

    /// Get unresolved references for specific files
    pub fn get_unresolved_refs_by_files(&self, file_paths: &[String]) -> Result<Vec<UnresolvedReference>, rusqlite::Error> {
        if file_paths.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = file_paths.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, from_node_id, reference_name, reference_kind, line, col, candidates, file_path, language
             FROM unresolved_refs WHERE file_path IN ({})",
            placeholders
        );

        let mut stmt = self.conn.prepare(&sql)?;

        let refs = stmt.query_map(rusqlite::params_from_iter(file_paths.iter().map(|s| s.as_str())), |row| {
            Ok(UnresolvedReference {
                id: row.get(0)?,
                from_node_id: row.get(1)?,
                reference_name: row.get(2)?,
                reference_kind: row.get(3)?,
                line: row.get(4)?,
                col: row.get(5)?,
                candidates: row.get::<_, Option<String>>(6)?
                    .and_then(|s| serde_json::from_str(&s).ok()),
                file_path: row.get(7)?,
                language: row.get(8)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(refs)
    }

    /// Delete resolved references
    pub fn delete_resolved_refs(&self, ids: &[i64]) -> Result<(), rusqlite::Error> {
        if ids.is_empty() {
            return Ok(());
        }

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("DELETE FROM unresolved_refs WHERE id IN ({})", placeholders);

        self.conn.execute(&sql, rusqlite::params_from_iter(ids.iter()))?;
        Ok(())
    }

    // ========================================================================
    // Search Operations (FTS5)
    // ========================================================================

    /// Search nodes using FTS5
    pub fn search_nodes(&self, query: &str, options: Option<&SearchOptions>) -> Result<Vec<SearchResult>, rusqlite::Error> {
        let default_opts = SearchOptions::default();
        let opts = options.unwrap_or(&default_opts);
        
        let sql = format!(
            "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path, n.language,
                    n.start_line, n.end_line, n.start_column, n.end_column,
                    n.docstring, n.signature, n.visibility, n.is_exported,
                    n.is_async, n.is_static, n.is_abstract, n.decorators,
                    n.type_parameters, n.return_type, n.updated_at,
                    rank
             FROM nodes n
             JOIN nodes_fts fts ON n.rowid = fts.rowid
             WHERE nodes_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2"
        );

        let mut stmt = self.conn.prepare(&sql)?;

        // Escape FTS5 special characters
        let escaped_query = escape_fts_query(query);

        let results = stmt.query_map(params![escaped_query, opts.limit], |row| {
            let node = Node {
                id: row.get(0)?,
                kind: NodeKind::from_str(&row.get::<_, String>(1)?).unwrap(),
                name: row.get(2)?,
                qualified_name: row.get(3)?,
                file_path: row.get(4)?,
                language: Language::from_str(&row.get::<_, String>(5)?).unwrap(),
                start_line: row.get(6)?,
                end_line: row.get(7)?,
                start_column: row.get(8)?,
                end_column: row.get(9)?,
                docstring: row.get(10)?,
                signature: row.get(11)?,
                visibility: row.get(12)?,
                is_exported: row.get::<_, i32>(13)? == 1,
                is_async: row.get::<_, i32>(14)? == 1,
                is_static: row.get::<_, i32>(15)? == 1,
                is_abstract: row.get::<_, i32>(16)? == 1,
                decorators: serde_json::from_str(&row.get::<_, String>(17)?).unwrap_or_default(),
                type_parameters: serde_json::from_str(&row.get::<_, String>(18)?).unwrap_or_default(),
                return_type: row.get(19)?,
                updated_at: row.get(20)?,
            };

            Ok(SearchResult {
                node,
                score: row.get::<_, f64>(21)?,
                matched_fields: vec!["name".to_string()],
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(results)
    }

    /// Full-text search wrapper for MCP tools
    pub fn full_text_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, rusqlite::Error> {
        let opts = SearchOptions {
            limit,
            kinds: None,
            file_pattern: None,
        };
        self.search_nodes(query, Some(&opts))
    }

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Get graph statistics
    pub fn get_stats(&self) -> Result<GraphStats, rusqlite::Error> {
        let node_count: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM nodes",
            [],
            |row| row.get(0)
        )?;

        let edge_count: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM edges",
            [],
            |row| row.get(0)
        )?;

        let file_count: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM files",
            [],
            |row| row.get(0)
        )?;

        let unresolved_count: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM unresolved_refs",
            [],
            |row| row.get(0)
        )?;

        Ok(GraphStats {
            node_count,
            edge_count,
            file_count,
            unresolved_ref_count: unresolved_count,
            db_size_bytes: 0, // Will be set by caller
            last_indexed_at: None,
        })
    }

    /// Get node and edge count
    pub fn get_node_and_edge_count(&self) -> Result<(u64, u64), rusqlite::Error> {
        let node_count: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM nodes",
            [],
            |row| row.get(0)
        )?;

        let edge_count: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM edges",
            [],
            |row| row.get(0)
        )?;

        Ok((node_count, edge_count))
    }

    /// Get all nodes from database
    pub fn get_all_nodes(&self) -> Result<Vec<Node>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, qualified_name, file_path, language,
             start_line, end_line, start_column, end_column,
             docstring, signature, visibility, is_exported,
             is_async, is_static, is_abstract, decorators,
             type_parameters, return_type, updated_at
             FROM nodes"
        )?;

        let nodes = stmt.query_map([], |row| {
            Ok(Node {
                id: row.get(0)?,
                kind: NodeKind::from_str(&row.get::<_, String>(1)?).unwrap(),
                name: row.get(2)?,
                qualified_name: row.get(3)?,
                file_path: row.get(4)?,
                language: Language::from_str(&row.get::<_, String>(5)?).unwrap(),
                start_line: row.get(6)?,
                end_line: row.get(7)?,
                start_column: row.get(8)?,
                end_column: row.get(9)?,
                docstring: row.get(10)?,
                signature: row.get(11)?,
                visibility: row.get(12)?,
                is_exported: row.get::<_, i32>(13)? == 1,
                is_async: row.get::<_, i32>(14)? == 1,
                is_static: row.get::<_, i32>(15)? == 1,
                is_abstract: row.get::<_, i32>(16)? == 1,
                decorators: serde_json::from_str(&row.get::<_, String>(17)?).unwrap_or_default(),
                type_parameters: serde_json::from_str(&row.get::<_, String>(18)?).unwrap_or_default(),
                return_type: row.get(19)?,
                updated_at: row.get(20)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(nodes)
    }

    // ========================================================================
    // Utility Operations
    // ========================================================================

    /// Clear all data
    pub fn clear(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch("
            DELETE FROM nodes;
            DELETE FROM edges;
            DELETE FROM files;
            DELETE FROM unresolved_refs;
        ")?;
        Ok(())
    }

    /// Get file-level dependents: files that depend on nodes in the given file
    pub fn get_file_dependents(&self, file_path: &str) -> Result<Vec<String>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT n2.file_path
             FROM edges e
             JOIN nodes n1 ON e.source = n1.id
             JOIN nodes n2 ON e.target = n2.id
             WHERE n1.file_path = ?1 AND n1.file_path != n2.file_path"
        )?;
        let rows = stmt.query_map(params![file_path], |row| row.get(0))?;
        rows.collect::<Result<Vec<String>, _>>()
            .map_err(|e| e.into())
    }

    /// Get node count by kind
    pub fn get_nodes_by_kind_counts(&self) -> Result<Vec<(String, u64)>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, COUNT(*) as cnt FROM nodes GROUP BY kind ORDER BY cnt DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
        })?;
        rows.collect()
    }

    /// Get distinct languages used in the project
    pub fn get_languages(&self) -> Result<Vec<String>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT language FROM files ORDER BY language"
        )?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect::<Result<Vec<String>, _>>()
            .map_err(|e| e.into())
    }

    /// Get file count by language
    pub fn get_file_counts_by_language(&self) -> Result<Vec<(String, u64)>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT language, COUNT(*) as cnt FROM files GROUP BY language ORDER BY cnt DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
        })?;
        rows.collect()
    }
}

/// Escape FTS5 special characters
fn escape_fts_query(query: &str) -> String {
    // Simple escaping - in production, use a proper FTS query builder
    query.replace("\"", "\"\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::db::connection::DatabaseConnection;
    use crate::db::schema::initialize_schema;

    #[test]
    fn test_insert_and_get_node() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = DatabaseConnection::initialize(db_path.to_str().unwrap()).unwrap();
        initialize_schema(db.get_conn()).unwrap();

        let queries = QueryBuilder::new(db.get_conn());

        let node = Node::new(
            "test-id".to_string(),
            NodeKind::Function,
            "testFunc".to_string(),
            "test.ts::testFunc".to_string(),
            "test.ts".to_string(),
            Language::TypeScript,
            1, 10, 0, 80,
        );

        queries.insert_node(&node).unwrap();

        let retrieved = queries.get_node_by_id("test-id").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "testFunc");
    }

    #[test]
    fn test_batch_insert() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = DatabaseConnection::initialize(db_path.to_str().unwrap()).unwrap();
        initialize_schema(db.get_conn()).unwrap();

        let queries = QueryBuilder::new(db.get_conn());

        let nodes = vec![
            Node::new("id1".to_string(), NodeKind::Function, "func1".to_string(), "test.ts::func1".to_string(), "test.ts".to_string(), Language::TypeScript, 1, 5, 0, 40),
            Node::new("id2".to_string(), NodeKind::Function, "func2".to_string(), "test.ts::func2".to_string(), "test.ts".to_string(), Language::TypeScript, 6, 10, 0, 40),
        ];

        let count = queries.insert_nodes_batch(&nodes).unwrap();
        assert_eq!(count, 2);

        let (node_count, _) = queries.get_node_and_edge_count().unwrap();
        assert_eq!(node_count, 2);
    }

    #[test]
    fn test_get_all_nodes() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = DatabaseConnection::initialize(db_path.to_str().unwrap()).unwrap();
        initialize_schema(db.get_conn()).unwrap();

        let queries = QueryBuilder::new(db.get_conn());

        let nodes = vec![
            Node::new("id1".to_string(), NodeKind::Function, "func1".to_string(), "test.ts::func1".to_string(), "test.ts".to_string(), Language::TypeScript, 1, 5, 0, 40),
            Node::new("id2".to_string(), NodeKind::Class, "MyClass".to_string(), "test.ts::MyClass".to_string(), "test.ts".to_string(), Language::TypeScript, 6, 20, 0, 40),
        ];

        queries.insert_nodes_batch(&nodes).unwrap();

        let all_nodes = queries.get_all_nodes().unwrap();
        assert_eq!(all_nodes.len(), 2);
    }

    #[test]
    fn test_edge_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = DatabaseConnection::initialize(db_path.to_str().unwrap()).unwrap();
        initialize_schema(db.get_conn()).unwrap();

        let queries = QueryBuilder::new(db.get_conn());

        // Create nodes
        let node_a = Node::new("node_a".to_string(), NodeKind::Function, "funcA".to_string(), "test.ts::funcA".to_string(), "test.ts".to_string(), Language::TypeScript, 1, 5, 0, 40);
        let node_b = Node::new("node_b".to_string(), NodeKind::Function, "funcB".to_string(), "test.ts::funcB".to_string(), "test.ts".to_string(), Language::TypeScript, 6, 10, 0, 40);

        queries.insert_node(&node_a).unwrap();
        queries.insert_node(&node_b).unwrap();

        // Create edge
        let edge = Edge::new("node_a".to_string(), "node_b".to_string(), EdgeKind::Calls);
        queries.insert_edge(&edge).unwrap();

        // Get outgoing edges
        let outgoing = queries.get_outgoing_edges("node_a").unwrap();
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].target, "node_b");

        // Get incoming edges
        let incoming = queries.get_incoming_edges("node_b").unwrap();
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].source, "node_a");
    }
}
