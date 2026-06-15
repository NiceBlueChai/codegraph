use crate::db::QueryBuilder;
use crate::types::*;
use log::{debug, info};
use std::collections::HashMap;
use std::str::FromStr;

/// Resolution context holds cached candidate nodes for efficient matching
pub struct ResolutionContext {
    /// All nodes indexed by name (lowercase for case-insensitive matching)
    nodes_by_name: HashMap<String, Vec<Node>>,
    /// All nodes indexed by qualified name
    nodes_by_qualified_name: HashMap<String, Vec<Node>>,
    /// All file nodes indexed by path
    nodes_by_file_path: HashMap<String, Node>,
    /// Nodes indexed by language family
    nodes_by_family: HashMap<String, Vec<Node>>,
}

impl ResolutionContext {
    pub fn new<'a>(queries: &QueryBuilder<'a>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut ctx = Self {
            nodes_by_name: HashMap::new(),
            nodes_by_qualified_name: HashMap::new(),
            nodes_by_file_path: HashMap::new(),
            nodes_by_family: HashMap::new(),
        };

        // Load all non-file nodes
        let nodes = queries.get_all_nodes()?;
        for node in nodes {
            // Index by name (lowercase)
            let name_lower = node.name.to_lowercase();
            ctx.nodes_by_name
                .entry(name_lower)
                .or_insert_with(Vec::new)
                .push(node.clone());

            // Index by qualified name
            if !node.qualified_name.is_empty() {
                let qn_lower = node.qualified_name.to_lowercase();
                ctx.nodes_by_qualified_name
                    .entry(qn_lower)
                    .or_insert_with(Vec::new)
                    .push(node.clone());
            }

            // Index file nodes by path
            if node.kind == NodeKind::File {
                ctx.nodes_by_file_path
                    .insert(node.file_path.clone(), node.clone());
            }

            // Index by language family
            if let Some(family) = get_language_family(&node.language) {
                ctx.nodes_by_family
                    .entry(family.to_string())
                    .or_insert_with(Vec::new)
                    .push(node);
            }
        }

        debug!(
            "Resolution context loaded: {} names, {} qualified names, {} files",
            ctx.nodes_by_name.len(),
            ctx.nodes_by_qualified_name.len(),
            ctx.nodes_by_file_path.len()
        );

        Ok(ctx)
    }

    /// Get candidates from the same language family
    fn get_family_candidates(&self, lang: &Language, name: &str) -> Vec<&Node> {
        if let Some(family) = get_language_family(lang) {
            if let Some(nodes) = self.nodes_by_family.get(family) {
                let name_lower = name.to_lowercase();
                return nodes.iter().filter(|n| n.name.to_lowercase() == name_lower).collect();
            }
        }
        // If no family or no match, return all candidates with this name
        self.nodes_by_name
            .get(&name.to_lowercase())
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }
}

/// Reference resolver implementing multi-strategy pipeline
pub struct Resolver<'a> {
    queries: QueryBuilder<'a>,
}

impl<'a> Resolver<'a> {
    pub fn new(queries: QueryBuilder<'a>) -> Self {
        Self { queries }
    }

    /// Resolve all unresolved references and create edges
    pub fn resolve_all(
        &self,
        unresolved_refs: &[UnresolvedReference],
    ) -> Result<Vec<ResolvedReference>, Box<dyn std::error::Error>> {
        info!("Resolving {} references...", unresolved_refs.len());

        let ctx = ResolutionContext::new(&self.queries)?;
        let mut resolved = Vec::new();

        for uref in unresolved_refs {
            if let Some(resolved_ref) = self.resolve_single(uref, &ctx) {
                debug!(
                    "Resolved '{}' -> '{}' (confidence: {:.2}, strategy: {:?})",
                    uref.reference_name,
                    resolved_ref.target_node_id,
                    resolved_ref.confidence,
                    resolved_ref.strategy
                );
                resolved.push(resolved_ref);
            }
        }

        info!("Resolved {} out of {} references", resolved.len(), unresolved_refs.len());

        // Create edges from resolved references
        self.create_edges_from_resolved(&resolved)?;

        Ok(resolved)
    }

    /// Resolve a single reference using multi-strategy pipeline
    fn resolve_single(&self, uref: &UnresolvedReference, ctx: &ResolutionContext) -> Option<ResolvedReference> {
        let lang = Language::from_str(&uref.language).ok()?;

        // Strategy 1: File path match (highest confidence)
        if let Some(result) = self.match_by_file_path(uref, ctx) {
            return Some(result);
        }

        // Strategy 2: Qualified name match
        if let Some(result) = self.match_by_qualified_name(uref, ctx, &lang) {
            return Some(result);
        }

        // Strategy 3: Method call pattern (obj.method, Class::method)
        if let Some(result) = self.match_method_call(uref, ctx, &lang) {
            return Some(result);
        }

        // Strategy 4: Exact name match (within language family)
        if let Some(result) = self.match_by_exact_name(uref, ctx, &lang) {
            return Some(result);
        }

        // Strategy 5: Fuzzy match (lowest confidence)
        if let Some(result) = self.match_fuzzy(uref, ctx, &lang) {
            return Some(result);
        }

        None
    }

    /// Strategy 1: Match by file path (e.g., "src/utils.ts" -> file node)
    fn match_by_file_path(&self, uref: &UnresolvedReference, ctx: &ResolutionContext) -> Option<ResolvedReference> {
        let ref_name = &uref.reference_name;

        // Check if reference looks like a file path
        if ref_name.contains('/') || ref_name.contains('\\') || ref_name.ends_with(".ts") ||
           ref_name.ends_with(".js") || ref_name.ends_with(".py") || ref_name.ends_with(".rs") {
            if let Some(file_node) = ctx.nodes_by_file_path.get(ref_name) {
                return Some(ResolvedReference {
                    from_node_id: uref.from_node_id.clone(),
                    target_node_id: file_node.id.clone(),
                    edge_kind: EdgeKind::References,
                    confidence: 0.95,
                    strategy: ResolutionStrategy::FilePath,
                    line: uref.line,
                    col: uref.col,
                });
            }
        }

        None
    }

    /// Strategy 2: Match by qualified name (e.g., "com.example.Foo", "std::vec::Vec")
    fn match_by_qualified_name(&self, uref: &UnresolvedReference, ctx: &ResolutionContext, lang: &Language) -> Option<ResolvedReference> {
        let ref_name = &uref.reference_name;

        // Try exact qualified name match
        if let Some(candidates) = ctx.nodes_by_qualified_name.get(&ref_name.to_lowercase()) {
            // Filter by language family
            for candidate in candidates {
                if self.same_language_family(lang, &candidate.language) {
                    return Some(ResolvedReference {
                        from_node_id: uref.from_node_id.clone(),
                        target_node_id: candidate.id.clone(),
                        edge_kind: self.infer_edge_kind(&uref.reference_kind),
                        confidence: 0.9,
                        strategy: ResolutionStrategy::QualifiedName,
                        line: uref.line,
                        col: uref.col,
                    });
                }
            }
        }

        None
    }

    /// Strategy 3: Match method call patterns (obj.method, Class::method)
    fn match_method_call(&self, uref: &UnresolvedReference, ctx: &ResolutionContext, lang: &Language) -> Option<ResolvedReference> {
        let ref_name = &uref.reference_name;

        // Parse method call patterns
        let parts: Vec<&str> = if ref_name.contains("::") {
            ref_name.split("::").collect()
        } else if ref_name.contains('.') {
            ref_name.split('.').collect()
        } else {
            return None;
        };

        if parts.len() >= 2 {
            let method_name = parts.last().unwrap();

            // Look for methods with this name in the same language family
            let candidates = ctx.get_family_candidates(lang, method_name);
            for candidate in candidates {
                if candidate.kind == NodeKind::Method || candidate.kind == NodeKind::Function {
                    return Some(ResolvedReference {
                        from_node_id: uref.from_node_id.clone(),
                        target_node_id: candidate.id.clone(),
                        edge_kind: EdgeKind::Calls,
                        confidence: 0.7,
                        strategy: ResolutionStrategy::MethodCall,
                        line: uref.line,
                        col: uref.col,
                    });
                }
            }
        }

        None
    }

    /// Strategy 4: Exact name match within language family
    fn match_by_exact_name(&self, uref: &UnresolvedReference, ctx: &ResolutionContext, lang: &Language) -> Option<ResolvedReference> {
        let ref_name = &uref.reference_name;

        let candidates = ctx.get_family_candidates(lang, ref_name);
        for candidate in &candidates {
            // Prefer exported/public symbols
            if candidate.is_exported {
                return Some(ResolvedReference {
                    from_node_id: uref.from_node_id.clone(),
                    target_node_id: candidate.id.clone(),
                    edge_kind: self.infer_edge_kind(&uref.reference_kind),
                    confidence: 0.6,
                    strategy: ResolutionStrategy::ExactName,
                    line: uref.line,
                    col: uref.col,
                });
            }
        }

        // Fall back to non-exported
        if let Some(candidate) = candidates.first() {
            return Some(ResolvedReference {
                from_node_id: uref.from_node_id.clone(),
                target_node_id: candidate.id.clone(),
                edge_kind: self.infer_edge_kind(&uref.reference_kind),
                confidence: 0.5,
                strategy: ResolutionStrategy::ExactName,
                line: uref.line,
                col: uref.col,
            });
        }

        None
    }

    /// Strategy 5: Fuzzy match (substring or case-insensitive)
    fn match_fuzzy(&self, uref: &UnresolvedReference, ctx: &ResolutionContext, lang: &Language) -> Option<ResolvedReference> {
        let ref_name_lower = uref.reference_name.to_lowercase();

        // Try substring match
        for (name, candidates) in &ctx.nodes_by_name {
            if name.contains(&ref_name_lower) || ref_name_lower.contains(name) {
                for candidate in candidates {
                    if self.same_language_family(lang, &candidate.language) {
                        return Some(ResolvedReference {
                            from_node_id: uref.from_node_id.clone(),
                            target_node_id: candidate.id.clone(),
                            edge_kind: self.infer_edge_kind(&uref.reference_kind),
                            confidence: 0.3,
                            strategy: ResolutionStrategy::Fuzzy,
                            line: uref.line,
                            col: uref.col,
                        });
                    }
                }
            }
        }

        None
    }

    /// Check if two languages belong to the same family
    fn same_language_family(&self, lang1: &Language, lang2: &Language) -> bool {
        match (get_language_family(lang1), get_language_family(lang2)) {
            (Some(f1), Some(f2)) => f1 == f2,
            (None, _) | (_, None) => true, // Unknown family allows cross-language
        }
    }

    /// Infer edge kind from reference kind string
    fn infer_edge_kind(&self, ref_kind: &str) -> EdgeKind {
        match ref_kind {
            "call" | "invocation" => EdgeKind::Calls,
            "import" | "require" => EdgeKind::Imports,
            "export" => EdgeKind::Exports,
            "extends" | "inherits" => EdgeKind::Extends,
            "implements" | "interface" => EdgeKind::Implements,
            "type" | "type_ref" => EdgeKind::TypeOf,
            "instantiation" | "new" => EdgeKind::Instantiates,
            _ => EdgeKind::References,
        }
    }

    /// Create edges from resolved references
    fn create_edges_from_resolved(&self, resolved: &[ResolvedReference]) -> Result<(), Box<dyn std::error::Error>> {
        let edges: Vec<Edge> = resolved.iter().map(|r| {
            let mut edge = Edge::new(
                r.from_node_id.clone(),
                r.target_node_id.clone(),
                r.edge_kind.clone(),
            );
            edge.line = Some(r.line);
            edge.column = Some(r.col);

            // Add confidence as metadata
            let mut metadata = HashMap::new();
            metadata.insert("confidence".to_string(), serde_json::json!(r.confidence));
            metadata.insert("strategy".to_string(), serde_json::json!(r.strategy.as_str()));
            edge.metadata = Some(metadata);

            edge
        }).collect();

        if !edges.is_empty() {
            self.queries.insert_edges_batch(&edges)?;
            info!("Created {} edges from resolved references", edges.len());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{DatabaseConnection, schema::initialize_schema};
    use tempfile::TempDir;

    #[test]
    fn test_resolver_basic() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = format!("{}/test.db", temp_dir.path().display());

        let db = Box::new(DatabaseConnection::initialize(&db_path).unwrap());
        initialize_schema(db.get_conn()).unwrap();
        let db: &'static DatabaseConnection = Box::leak(db);
        let queries = QueryBuilder::new(db.get_conn());

        let resolver = Resolver::new(queries);

        // Test with empty unresolved refs
        let result = resolver.resolve_all(&[]).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_language_family() {
        assert_eq!(get_language_family(&Language::Java), Some("jvm"));
        assert_eq!(get_language_family(&Language::Kotlin), Some("jvm"));
        assert_eq!(get_language_family(&Language::TypeScript), Some("web"));
        assert_eq!(get_language_family(&Language::Python), None);
    }
}
