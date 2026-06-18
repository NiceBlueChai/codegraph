use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use log::info;
use sha2::{Sha256, Digest};
use ignore::WalkBuilder;
use rayon::prelude::*;
use crate::types::*;
use crate::db::QueryBuilder;
use crate::extraction::parser::CodeParser;
use crate::core::resolver::Resolver;

/// Result of parsing a single file (for parallel processing)
struct ParsedFile {
    rel_path: String,
    hash: String,
    content_len: usize,
    extraction: ExtractionResult,
    error: Option<String>,
}

pub struct Indexer<'a> {
    queries: QueryBuilder<'a>,
    project_root: String,
}

impl<'a> Indexer<'a> {
    pub fn new(queries: QueryBuilder<'a>, project_root: &str) -> Self {
        Self {
            queries,
            project_root: project_root.to_string(),
        }
    }

    pub fn index_all(&self) -> Result<IndexResult, Box<dyn std::error::Error>> {
        info!("Starting full index of {}", self.project_root);
        let start = std::time::Instant::now();
        let mut result = IndexResult {
            success: true, files_indexed: 0, files_skipped: 0,
            files_errored: 0, nodes_created: 0, edges_created: 0,
            errors: Vec::new(), duration_ms: 0,
        };

        let files = self.scan_directory()?;
        info!("Found {} files", files.len());

        // Phase 1: Check which files need re-indexing (sequential - needs DB access)
        let mut files_to_parse: Vec<(PathBuf, String)> = Vec::new(); // (path, rel_path)

        for file_path in &files {
            let rel_path = match self.make_relative(file_path) {
                Ok(p) => p,
                Err(_) => continue,
            };

            // Quick hash check to skip unchanged files
            match crate::util::read_file_content(file_path) {
                Ok(content) => {
                    let hash = self.hash_content(&content);
                    if let Some(existing) = self.queries.get_file_by_path(&rel_path)? {
                        if existing.content_hash == hash {
                            result.files_skipped += 1;
                            continue;
                        }
                        // Delete old nodes for modified file
                        self.queries.delete_nodes_for_file(&rel_path).ok();
                    }
                    files_to_parse.push((file_path.clone(), rel_path));
                }
                Err(e) => {
                    result.files_errored += 1;
                    result.errors.push(IndexError {
                        message: format!("{}", e),
                        severity: "error".into(),
                        file_path: Some(file_path.display().to_string()),
                    });
                }
            }
        }

        info!("Parsing {} files in parallel ({} skipped)", files_to_parse.len(), result.files_skipped);

        // Phase 2: Parse files in parallel using rayon
        let parsed_files: Vec<ParsedFile> = files_to_parse
            .par_iter()
            .map(|(file_path, rel_path)| {
                let content = match crate::util::read_file_content(file_path) {
                    Ok(c) => c,
                    Err(e) => {
                        return ParsedFile {
                            rel_path: rel_path.clone(),
                            hash: String::new(),
                            content_len: 0,
                            extraction: ExtractionResult::new(),
                            error: Some(e.to_string()),
                        };
                    }
                };

                let hash = {
                    let mut hasher = Sha256::new();
                    hasher.update(content.as_bytes());
                    format!("{:x}", hasher.finalize())
                };

                let mut parser = CodeParser::new();
                let mut extraction = parser.parse(rel_path, &content);

                // Update file paths in unresolved refs
                for uref in &mut extraction.unresolved_refs {
                    uref.file_path = rel_path.clone();
                }

                ParsedFile {
                    rel_path: rel_path.clone(),
                    hash,
                    content_len: content.len(),
                    extraction,
                    error: None,
                }
            })
            .collect();

        // Phase 3: Store results in database (sequential - SQLite write constraint)
        let mut all_unresolved_refs = Vec::new();

        for parsed in parsed_files {
            if let Some(err) = parsed.error {
                result.files_errored += 1;
                result.errors.push(IndexError {
                    message: err,
                    severity: "error".into(),
                    file_path: Some(parsed.rel_path),
                });
                continue;
            }

            if !parsed.extraction.nodes.is_empty() {
                self.queries.insert_nodes_batch(&parsed.extraction.nodes)?;
            }
            if !parsed.extraction.edges.is_empty() {
                self.queries.insert_edges_batch(&parsed.extraction.edges)?;
            }

            all_unresolved_refs.extend(parsed.extraction.unresolved_refs);

            let file_rec = FileRecord {
                path: parsed.rel_path,
                content_hash: parsed.hash,
                language: parsed.extraction.nodes.first()
                    .map(|n| n.language.clone())
                    .unwrap_or(Language::Unknown),
                size: parsed.content_len as u64,
                modified_at: chrono::Utc::now().timestamp_millis(),
                indexed_at: chrono::Utc::now().timestamp_millis(),
                node_count: parsed.extraction.nodes.len() as u32,
                errors: parsed.extraction.errors.clone(),
            };
            self.queries.upsert_file(&file_rec)?;

            result.files_indexed += 1;
            result.nodes_created += parsed.extraction.nodes.len();
            result.edges_created += parsed.extraction.edges.len();
        }

        let go_edges = self.synthesize_go_implicit_interface_edges()?;
        if !go_edges.is_empty() {
            result.edges_created += self.queries.insert_edges_batch(&go_edges)?;
        }

        // Phase 4: Resolve references
        if !all_unresolved_refs.is_empty() {
            info!("Resolving {} unresolved references...", all_unresolved_refs.len());
            let resolver = Resolver::new(QueryBuilder::new(self.queries.get_conn()));
            match resolver.resolve_all(&all_unresolved_refs) {
                Ok(resolved) => {
                    let resolved_count = resolved.len();
                    result.edges_created += resolved_count;
                    info!("Resolved {} references into edges", resolved_count);
                }
                Err(e) => {
                    info!("Reference resolution encountered errors: {}", e);
                }
            }
        }

        result.duration_ms = start.elapsed().as_millis() as u64;
        info!("Indexed {} files, {} nodes, {} edges in {}ms",
            result.files_indexed, result.nodes_created, result.edges_created, result.duration_ms);
        Ok(result)
    }

    fn synthesize_go_implicit_interface_edges(&self) -> Result<Vec<Edge>, Box<dyn std::error::Error>> {
        let nodes = self.queries.get_all_nodes()?;
        let mut types_by_package_name: HashMap<(String, String), Node> = HashMap::new();
        for node in &nodes {
            if node.language != Language::Go {
                continue;
            }
            if matches!(node.kind, NodeKind::Struct | NodeKind::Interface) {
                types_by_package_name.insert(
                    (go_package_dir(&node.file_path), node.name.clone()),
                    node.clone(),
                );
            }
        }

        let mut edges = Vec::new();
        let mut seen = HashSet::new();
        for node in &nodes {
            for edge in self.queries.get_outgoing_edges(&node.id)? {
                seen.insert(go_edge_key(&edge.source, &edge.target, &edge.kind));
            }
        }
        let mut struct_methods: HashMap<String, Vec<Node>> = HashMap::new();
        let mut interface_methods: HashMap<String, Vec<Node>> = HashMap::new();

        for method in nodes.iter().filter(|node| {
            node.language == Language::Go && node.kind == NodeKind::Method
        }) {
            let Some(owner_name) = go_method_owner_name(&method.qualified_name) else {
                continue;
            };
            let owner_key = (go_package_dir(&method.file_path), owner_name);
            let Some(owner) = types_by_package_name.get(&owner_key) else {
                continue;
            };

            match owner.kind {
                NodeKind::Struct => {
                    let contains_key = go_edge_key(&owner.id, &method.id, &EdgeKind::Contains);
                    if seen.insert(contains_key) {
                        edges.push(Edge::new(
                            owner.id.clone(),
                            method.id.clone(),
                            EdgeKind::Contains,
                        ));
                    }
                    struct_methods
                        .entry(owner.id.clone())
                        .or_default()
                        .push(method.clone());
                }
                NodeKind::Interface => {
                    interface_methods
                        .entry(owner.id.clone())
                        .or_default()
                        .push(method.clone());
                }
                _ => {}
            }
        }

        for (interface_id, wanted_methods) in &interface_methods {
            if wanted_methods.is_empty() {
                continue;
            }
            let Some(interface_node) = nodes.iter().find(|node| node.id == *interface_id) else {
                continue;
            };
            let wanted_names = wanted_methods
                .iter()
                .map(|method| method.name.as_str())
                .collect::<HashSet<_>>();
            for (struct_id, methods) in &struct_methods {
                let Some(struct_node) = nodes.iter().find(|node| node.id == *struct_id) else {
                    continue;
                };
                if go_package_dir(&struct_node.file_path) != go_package_dir(&interface_node.file_path) {
                    continue;
                }
                let have_names = methods
                    .iter()
                    .map(|method| method.name.as_str())
                    .collect::<HashSet<_>>();
                if !wanted_names.iter().all(|name| have_names.contains(name)) {
                    continue;
                }

                let implements_key = go_edge_key(&struct_node.id, &interface_node.id, &EdgeKind::Implements);
                if seen.insert(implements_key) {
                    let mut edge = Edge::new(
                        struct_node.id.clone(),
                        interface_node.id.clone(),
                        EdgeKind::Implements,
                    );
                    edge.provenance = Some(Provenance::Heuristic);
                    edges.push(edge);
                }

                for wanted in wanted_methods {
                    for implementation in methods.iter().filter(|method| method.name == wanted.name) {
                        let bridge_key = go_edge_key(&wanted.id, &implementation.id, &EdgeKind::Calls);
                        if !seen.insert(bridge_key) {
                            continue;
                        }
                        let mut edge = Edge::new(
                            wanted.id.clone(),
                            implementation.id.clone(),
                            EdgeKind::Calls,
                        );
                        edge.line = Some(wanted.start_line);
                        edge.provenance = Some(Provenance::Heuristic);
                        edges.push(edge);
                    }
                }
            }
        }

        Ok(edges)
    }

    /// Incremental sync - only process changed files and detect deletions
    pub fn sync(&self) -> Result<SyncResult, Box<dyn std::error::Error>> {
        info!("Starting incremental sync of {}", self.project_root);
        let start = std::time::Instant::now();
        let mut result = SyncResult {
            files_checked: 0,
            files_added: 0,
            files_modified: 0,
            files_removed: 0,
            nodes_updated: 0,
            duration_ms: 0,
            changed_file_paths: Some(Vec::new()),
        };

        // Get current files on disk
        let disk_files = self.scan_directory()?;
        let disk_file_set: std::collections::HashSet<String> = disk_files
            .iter()
            .filter_map(|p| self.make_relative(p).ok())
            .collect();

        // Get indexed files from DB
        let db_files = self.queries.get_all_files()?;
        let _db_file_set: std::collections::HashSet<String> = db_files
            .iter()
            .map(|f| f.path.clone())
            .collect();

        // Detect deleted files
        for db_file in &db_files {
            if !disk_file_set.contains(&db_file.path) {
                info!("Removing deleted file: {}", db_file.path);
                self.queries.delete_nodes_for_file(&db_file.path).ok();
                self.queries.delete_file(&db_file.path).ok();
                result.files_removed += 1;
                if let Some(ref mut paths) = result.changed_file_paths {
                    paths.push(db_file.path.clone());
                }
            }
        }

        // Process added/modified files
        let mut all_unresolved_refs = Vec::new();

        for file_path in &disk_files {
            let rel_path = self.make_relative(file_path)?;
            result.files_checked += 1;

            let content = match crate::util::read_file_content(file_path) {
                Ok(c) => c,
                Err(e) => {
                    info!("Failed to read {}: {}", file_path.display(), e);
                    continue;
                }
            };

            let hash = self.hash_content(&content);

            // Check if unchanged
            if let Some(existing) = self.queries.get_file_by_path(&rel_path)? {
                if existing.content_hash == hash {
                    continue; // Skip unchanged files
                }
                // Delete old nodes for modified file
                self.queries.delete_nodes_for_file(&rel_path).ok();
                result.files_modified += 1;
            } else {
                result.files_added += 1;
            }

            // Parse and store
            let mut parser = CodeParser::new();
            let extraction = parser.parse(&rel_path, &content);

            if !extraction.nodes.is_empty() {
                self.queries.insert_nodes_batch(&extraction.nodes)?;
            }
            if !extraction.edges.is_empty() {
                self.queries.insert_edges_batch(&extraction.edges)?;
            }

            // Collect unresolved references
            for mut uref in extraction.unresolved_refs {
                uref.file_path = rel_path.clone();
                all_unresolved_refs.push(uref);
            }

            // Store file record
            let file_rec = FileRecord {
                path: rel_path.clone(),
                content_hash: hash,
                language: extraction.nodes.first().map(|n| n.language.clone()).unwrap_or(Language::Unknown),
                size: content.len() as u64,
                modified_at: chrono::Utc::now().timestamp_millis(),
                indexed_at: chrono::Utc::now().timestamp_millis(),
                node_count: extraction.nodes.len() as u32,
                errors: extraction.errors.clone(),
            };
            self.queries.upsert_file(&file_rec)?;

            result.nodes_updated += extraction.nodes.len();
            if let Some(ref mut paths) = result.changed_file_paths {
                paths.push(rel_path);
            }
        }

        let go_edges = self.synthesize_go_implicit_interface_edges()?;
        if !go_edges.is_empty() {
            self.queries.insert_edges_batch(&go_edges)?;
        }

        // Resolve references for changed files
        if !all_unresolved_refs.is_empty() {
            info!("Resolving {} unresolved references...", all_unresolved_refs.len());
            let resolver = Resolver::new(QueryBuilder::new(self.queries.get_conn()));
            match resolver.resolve_all(&all_unresolved_refs) {
                Ok(resolved) => {
                    let resolved_count = resolved.len();
                    result.nodes_updated += resolved_count;
                    info!("Resolved {} references into edges", resolved_count);
                }
                Err(e) => {
                    info!("Reference resolution encountered errors: {}", e);
                }
            }
        }

        result.duration_ms = start.elapsed().as_millis() as u64;
        info!("Sync complete: {} added, {} modified, {} removed in {}ms",
            result.files_added, result.files_modified, result.files_removed, result.duration_ms);
        Ok(result)
    }

    /// Sync specific files (for watcher integration)
    pub fn sync_files(&self, file_paths: &[String]) -> Result<SyncResult, Box<dyn std::error::Error>> {
        info!("Syncing {} specific files", file_paths.len());
        let start = std::time::Instant::now();
        let mut result = SyncResult {
            files_checked: file_paths.len(),
            files_added: 0,
            files_modified: 0,
            files_removed: 0,
            nodes_updated: 0,
            duration_ms: 0,
            changed_file_paths: Some(Vec::new()),
        };

        let mut all_unresolved_refs = Vec::new();

        for rel_path in file_paths {
            let full_path = Path::new(&self.project_root).join(rel_path);

            // Check if file exists
            if !full_path.exists() {
                // File was deleted
                self.queries.delete_nodes_for_file(rel_path).ok();
                self.queries.delete_file(rel_path).ok();
                result.files_removed += 1;
                if let Some(ref mut paths) = result.changed_file_paths {
                    paths.push(rel_path.clone());
                }
                continue;
            }

            let content = match crate::util::read_file_content(&full_path) {
                Ok(c) => c,
                Err(e) => {
                    info!("Failed to read {}: {}", full_path.display(), e);
                    continue;
                }
            };

            let hash = self.hash_content(&content);

            // Check if unchanged
            if let Some(existing) = self.queries.get_file_by_path(rel_path)? {
                if existing.content_hash == hash {
                    continue;
                }
                self.queries.delete_nodes_for_file(rel_path).ok();
                result.files_modified += 1;
            } else {
                result.files_added += 1;
            }

            // Parse and store
            let mut parser = CodeParser::new();
            let extraction = parser.parse(rel_path, &content);

            if !extraction.nodes.is_empty() {
                self.queries.insert_nodes_batch(&extraction.nodes)?;
            }
            if !extraction.edges.is_empty() {
                self.queries.insert_edges_batch(&extraction.edges)?;
            }

            for mut uref in extraction.unresolved_refs {
                uref.file_path = rel_path.clone();
                all_unresolved_refs.push(uref);
            }

            let file_rec = FileRecord {
                path: rel_path.clone(),
                content_hash: hash,
                language: extraction.nodes.first().map(|n| n.language.clone()).unwrap_or(Language::Unknown),
                size: content.len() as u64,
                modified_at: chrono::Utc::now().timestamp_millis(),
                indexed_at: chrono::Utc::now().timestamp_millis(),
                node_count: extraction.nodes.len() as u32,
                errors: extraction.errors.clone(),
            };
            self.queries.upsert_file(&file_rec)?;

            result.nodes_updated += extraction.nodes.len();
            if let Some(ref mut paths) = result.changed_file_paths {
                paths.push(rel_path.clone());
            }
        }

        // Resolve references
        if !all_unresolved_refs.is_empty() {
            let resolver = Resolver::new(QueryBuilder::new(self.queries.get_conn()));
            match resolver.resolve_all(&all_unresolved_refs) {
                Ok(resolved) => {
                    result.nodes_updated += resolved.len();
                }
                Err(e) => {
                    info!("Reference resolution encountered errors: {}", e);
                }
            }
        }

        result.duration_ms = start.elapsed().as_millis() as u64;
        Ok(result)
    }

    fn scan_directory(&self) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
        let mut files = Vec::new();
        let walker = WalkBuilder::new(&self.project_root)
            .hidden(false)
            .build();

        for entry in walker {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if Language::from_extension(ext).is_some() {
                            files.push(path.to_path_buf());
                        }
                    }
                }
            }
        }
        Ok(files)
    }

    fn hash_content(&self, content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn make_relative(&self, path: &Path) -> Result<String, Box<dyn std::error::Error>> {
        let rel = path.strip_prefix(&self.project_root)?;
        Ok(rel.to_string_lossy().replace('\\', "/"))
    }
}

fn go_package_dir(file_path: &str) -> String {
    file_path
        .replace('\\', "/")
        .rsplit_once('/')
        .map(|(dir, _)| dir.to_string())
        .unwrap_or_default()
}

fn go_method_owner_name(qualified_name: &str) -> Option<String> {
    let symbol = qualified_name.rsplit_once("::")?.1;
    let (owner, _) = symbol.rsplit_once('.')?;
    (!owner.is_empty()).then(|| owner.to_string())
}

fn go_edge_key(source: &str, target: &str, kind: &EdgeKind) -> String {
    format!("{}>{}:{}", source, target, kind.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::db::{DatabaseConnection};
    use crate::db::schema::initialize_schema;

    #[test]
    fn test_hash() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Box::new(DatabaseConnection::initialize(db_path.to_str().unwrap()).unwrap());
        initialize_schema(db.get_conn()).unwrap();
        let db: &'static DatabaseConnection = Box::leak(db);
        let queries = QueryBuilder::new(db.get_conn());
        let indexer = Indexer::new(queries, ".");

        let h1 = indexer.hash_content("hello");
        let h2 = indexer.hash_content("hello");
        let h3 = indexer.hash_content("world");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }
}
