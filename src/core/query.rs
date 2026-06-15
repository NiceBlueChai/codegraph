use crate::db::QueryBuilder;
use crate::types::*;
use std::collections::{HashMap, HashSet, VecDeque};
use log::debug;

/// Graph traverser providing BFS/DFS traversal and specialized queries
pub struct GraphTraverser<'a> {
    pub queries: QueryBuilder<'a>,
}

impl<'a> GraphTraverser<'a> {
    pub fn new(queries: QueryBuilder<'a>) -> Self {
        Self { queries }
    }

    /// Perform BFS traversal from a starting node
    pub fn traverse_bfs(&self, start_id: &str, options: TraversalOptions) -> Result<Subgraph, Box<dyn std::error::Error>> {
        debug!("Starting BFS traversal from node {}", start_id);

        let mut subgraph = Subgraph::new();
        let mut visited = HashSet::new();
        let mut queue: VecDeque<(String, Option<Edge>, usize)> = VecDeque::new();

        // Get start node
        let start_node_result = self.queries.get_node_by_id(start_id)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        if let Some(start_node) = start_node_result {
            subgraph.nodes.insert(start_id.to_string(), start_node);
            visited.insert(start_id.to_string());
            queue.push_back((start_id.to_string(), None, 0));
        } else {
            return Ok(subgraph);
        }

        while let Some((current_id, edge, depth)) = queue.pop_front() {
            // Add edge if present
            if let Some(e) = edge {
                subgraph.edges.push(e);
            }

            // Stop if we've reached max depth or limit
            if depth >= options.max_depth {
                continue;
            }
            if let Some(limit) = options.limit {
                if subgraph.nodes.len() >= limit {
                    break;
                }
            }

            // Get adjacent edges based on direction
            let adjacent_edges = self.get_adjacent_edges(&current_id, &options.direction, &options.edge_kinds)?;

            // Sort edges by priority (structural edges first)
            let mut sorted_edges = adjacent_edges;
            sorted_edges.sort_by(|a, b| {
                let priority_a = edge_priority(a);
                let priority_b = edge_priority(b);
                priority_a.cmp(&priority_b)
            });

            // Batch fetch unvisited neighbors
            let neighbor_ids: Vec<String> = sorted_edges.iter()
                .map(|e| {
                    if options.direction == TraversalDirection::Incoming {
                        e.source.clone()
                    } else {
                        e.target.clone()
                    }
                })
                .filter(|id| !visited.contains(id))
                .collect();

            if !neighbor_ids.is_empty() {
                let neighbor_nodes = self.queries.get_nodes_by_ids(&neighbor_ids)?;

                for edge in sorted_edges {
                    let next_id = if options.direction == TraversalDirection::Incoming {
                        edge.source.clone()
                    } else {
                        edge.target.clone()
                    };

                    if !visited.contains(&next_id) {
                        if let Some(node) = neighbor_nodes.get(&next_id) {
                            // Check node kind filter if specified
                            if let Some(ref kinds) = options.node_kinds {
                                if !kinds.contains(&node.kind) {
                                    continue;
                                }
                            }

                            visited.insert(next_id.clone());
                            subgraph.nodes.insert(next_id.clone(), node.clone());
                            queue.push_back((next_id, Some(edge), depth + 1));
                        }
                    }
                }
            }
        }

        debug!("BFS traversal complete: {} nodes, {} edges", subgraph.nodes.len(), subgraph.edges.len());
        Ok(subgraph)
    }

    /// Perform DFS traversal from a starting node
    pub fn traverse_dfs(&self, start_id: &str, options: TraversalOptions) -> Result<Subgraph, Box<dyn std::error::Error>> {
        debug!("Starting DFS traversal from node {}", start_id);

        let mut subgraph = Subgraph::new();
        let mut visited = HashSet::new();
        let mut stack: Vec<(String, Option<Edge>, usize)> = Vec::new();

        // Get start node
        let start_node_result = self.queries.get_node_by_id(start_id)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        if let Some(start_node) = start_node_result {
            subgraph.nodes.insert(start_id.to_string(), start_node);
            visited.insert(start_id.to_string());
            stack.push((start_id.to_string(), None, 0));
        } else {
            return Ok(subgraph);
        }

        while let Some((current_id, edge, depth)) = stack.pop() {
            // Add edge if present
            if let Some(e) = edge {
                subgraph.edges.push(e);
            }

            // Stop if we've reached max depth or limit
            if depth >= options.max_depth {
                continue;
            }
            if let Some(limit) = options.limit {
                if subgraph.nodes.len() >= limit {
                    break;
                }
            }

            // Get adjacent edges based on direction
            let adjacent_edges = self.get_adjacent_edges(&current_id, &options.direction, &options.edge_kinds)?;

            // Sort edges by priority (structural edges first)
            let mut sorted_edges = adjacent_edges;
            sorted_edges.sort_by(|a, b| {
                let priority_a = edge_priority(a);
                let priority_b = edge_priority(b);
                priority_a.cmp(&priority_b)
            });

            // Process neighbors in reverse order (so first neighbor is processed first)
            for edge in sorted_edges.into_iter().rev() {
                let next_id = if options.direction == TraversalDirection::Incoming {
                    edge.source.clone()
                } else {
                    edge.target.clone()
                };

                if !visited.contains(&next_id) {
                    let node_result = self.queries.get_node_by_id(&next_id)
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                    if let Some(node) = node_result {
                        // Check node kind filter if specified
                        if let Some(ref kinds) = options.node_kinds {
                            if !kinds.contains(&node.kind) {
                                continue;
                            }
                        }

                        visited.insert(next_id.clone());
                        subgraph.nodes.insert(next_id.clone(), node);
                        stack.push((next_id, Some(edge), depth + 1));
                    }
                }
            }
        }

        debug!("DFS traversal complete: {} nodes, {} edges", subgraph.nodes.len(), subgraph.edges.len());
        Ok(subgraph)
    }

    /// Get callers of a node (nodes that call this node)
    pub fn get_callers(&self, node_id: &str, max_depth: usize) -> Result<Vec<(Node, Edge)>, Box<dyn std::error::Error>> {
        debug!("Finding callers of node {}", node_id);

        let options = TraversalOptions {
            max_depth,
            edge_kinds: Some(vec![EdgeKind::Calls]),
            node_kinds: None,
            direction: TraversalDirection::Incoming,
            limit: Some(100),
        };

        let subgraph = self.traverse_bfs(node_id, options)?;

        // Extract caller nodes and edges
        let mut callers = Vec::new();
        for edge in &subgraph.edges {
            if edge.target == node_id && edge.kind == EdgeKind::Calls {
                if let Some(node) = subgraph.nodes.get(&edge.source) {
                    callers.push((node.clone(), edge.clone()));
                }
            }
        }

        Ok(callers)
    }

    /// Get callees of a node (nodes that this node calls)
    pub fn get_callees(&self, node_id: &str, max_depth: usize) -> Result<Vec<(Node, Edge)>, Box<dyn std::error::Error>> {
        debug!("Finding callees of node {}", node_id);

        let options = TraversalOptions {
            max_depth,
            edge_kinds: Some(vec![EdgeKind::Calls]),
            node_kinds: None,
            direction: TraversalDirection::Outgoing,
            limit: Some(100),
        };

        let subgraph = self.traverse_bfs(node_id, options)?;

        // Extract callee nodes and edges
        let mut callees = Vec::new();
        for edge in &subgraph.edges {
            if edge.source == node_id && edge.kind == EdgeKind::Calls {
                if let Some(node) = subgraph.nodes.get(&edge.target) {
                    callees.push((node.clone(), edge.clone()));
                }
            }
        }

        Ok(callees)
    }

    /// Get impact radius - all nodes affected by changes to this node
    pub fn get_impact_radius(&self, node_id: &str, max_depth: usize) -> Result<Subgraph, Box<dyn std::error::Error>> {
        debug!("Calculating impact radius for node {}", node_id);

        let options = TraversalOptions {
            max_depth,
            edge_kinds: Some(vec![
                EdgeKind::Calls,
                EdgeKind::References,
                EdgeKind::Extends,
                EdgeKind::Implements,
                EdgeKind::TypeOf,
            ]),
            node_kinds: None,
            direction: TraversalDirection::Incoming,
            limit: Some(500),
        };

        self.traverse_bfs(node_id, options)
    }

    /// Find shortest path between two nodes
    pub fn find_path(&self, from_id: &str, to_id: &str, edge_kinds: Vec<EdgeKind>) -> Result<Option<Vec<(Node, Edge)>>, Box<dyn std::error::Error>> {
        debug!("Finding path from {} to {}", from_id, to_id);

        let options = TraversalOptions {
            max_depth: 10,
            edge_kinds: Some(edge_kinds),
            node_kinds: None,
            direction: TraversalDirection::Outgoing,
            limit: Some(1000),
        };

        let subgraph = self.traverse_bfs(from_id, options)?;

        // Check if target is reachable
        if !subgraph.nodes.contains_key(to_id) {
            return Ok(None);
        }

        // Reconstruct path from target back to source using BFS edges
        let mut path = Vec::new();
        let mut current_id = to_id.to_string();

        // Build incoming edge adjacency list
        let mut incoming_edges: HashMap<String, Vec<Edge>> = HashMap::new();
        for edge in &subgraph.edges {
            incoming_edges.entry(edge.target.clone()).or_insert_with(Vec::new).push(edge.clone());
        }

        // Trace back from target to source
        while current_id != from_id {
            if let Some(edges) = incoming_edges.get(&current_id) {
                let mut found = false;
                for edge in edges {
                    if subgraph.nodes.contains_key(&edge.source) {
                        path.push((subgraph.nodes[&edge.source].clone(), edge.clone()));
                        current_id = edge.source.clone();
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Ok(None);
                }
            } else {
                return Ok(None);
            }
        }

        path.reverse();
        Ok(Some(path))
    }

    /// Get type hierarchy for a node (parent classes, interfaces, etc.)
    pub fn get_type_hierarchy(&self, node_id: &str) -> Result<Subgraph, Box<dyn std::error::Error>> {
        debug!("Getting type hierarchy for node {}", node_id);

        let options = TraversalOptions {
            max_depth: 5,
            edge_kinds: Some(vec![EdgeKind::Extends, EdgeKind::Implements]),
            node_kinds: Some(vec![NodeKind::Class, NodeKind::Interface, NodeKind::Trait, NodeKind::Protocol]),
            direction: TraversalDirection::Both,
            limit: Some(100),
        };

        self.traverse_bfs(node_id, options)
    }

    /// Helper: Get adjacent edges based on direction and edge kinds
    fn get_adjacent_edges(&self, node_id: &str, direction: &TraversalDirection, edge_kinds: &Option<Vec<EdgeKind>>) -> Result<Vec<Edge>, Box<dyn std::error::Error>> {
        let mut edges = Vec::new();

        match direction {
            TraversalDirection::Outgoing => {
                let outgoing = self.queries.get_outgoing_edges(node_id)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                edges.extend(outgoing);
            }
            TraversalDirection::Incoming => {
                let incoming = self.queries.get_incoming_edges(node_id)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                edges.extend(incoming);
            }
            TraversalDirection::Both => {
                let outgoing = self.queries.get_outgoing_edges(node_id)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                let incoming = self.queries.get_incoming_edges(node_id)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                edges.extend(outgoing);
                edges.extend(incoming);
            }
        }

        // Filter by edge kinds if specified
        if let Some(ref kinds) = edge_kinds {
            edges.retain(|e| kinds.contains(&e.kind));
        }

        Ok(edges)
    }
}

/// Assign priority to edge types (lower number = higher priority)
fn edge_priority(edge: &Edge) -> u8 {
    match edge.kind {
        EdgeKind::Contains => 0,
        EdgeKind::Calls => 1,
        EdgeKind::Imports => 2,
        EdgeKind::Exports => 3,
        EdgeKind::Extends => 4,
        EdgeKind::Implements => 5,
        EdgeKind::References => 6,
        EdgeKind::TypeOf => 7,
        EdgeKind::Returns => 8,
        EdgeKind::Instantiates => 9,
        EdgeKind::Overrides => 10,
        EdgeKind::Decorates => 11,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{DatabaseConnection, schema::initialize_schema};
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, GraphTraverser<'static>) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = format!("{}/test.db", temp_dir.path().display());
        let db = Box::new(DatabaseConnection::initialize(&db_path).unwrap());
        initialize_schema(db.get_conn()).unwrap();
        let db: &'static DatabaseConnection = Box::leak(db);
        let queries = QueryBuilder::new(db.get_conn());
        let traverser = GraphTraverser::new(queries);

        // Create test nodes
        traverser.queries.insert_node(&Node::new(
            "func_a".to_string(), NodeKind::Function, "funcA".to_string(),
            "test.ts::funcA".to_string(), "test.ts".to_string(), Language::TypeScript,
            1, 10, 0, 40,
        )).unwrap();
        traverser.queries.insert_node(&Node::new(
            "func_b".to_string(), NodeKind::Function, "funcB".to_string(),
            "test.ts::funcB".to_string(), "test.ts".to_string(), Language::TypeScript,
            11, 20, 0, 40,
        )).unwrap();
        traverser.queries.insert_node(&Node::new(
            "func_c".to_string(), NodeKind::Function, "funcC".to_string(),
            "test.ts::funcC".to_string(), "test.ts".to_string(), Language::TypeScript,
            21, 30, 0, 40,
        )).unwrap();

        // Create edges: func_a -> func_b -> func_c
        traverser.queries.insert_edge(&Edge::new("func_a".to_string(), "func_b".to_string(), EdgeKind::Calls)).unwrap();
        traverser.queries.insert_edge(&Edge::new("func_b".to_string(), "func_c".to_string(), EdgeKind::Calls)).unwrap();

        (temp_dir, traverser)
    }

    #[test]
    fn test_bfs_traversal() {
        let (_temp_dir, traverser) = setup_test_db();

        let options = TraversalOptions {
            max_depth: 2,
            edge_kinds: None,
            node_kinds: None,
            direction: TraversalDirection::Outgoing,
            limit: Some(10),
        };

        let subgraph = traverser.traverse_bfs("func_a", options).unwrap();
        assert_eq!(subgraph.nodes.len(), 3);
        assert_eq!(subgraph.edges.len(), 2);
    }

    #[test]
    fn test_get_callers() {
        let (_temp_dir, traverser) = setup_test_db();

        let callers = traverser.get_callers("func_b", 1).unwrap();
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].0.id, "func_a");
    }

    #[test]
    fn test_get_callees() {
        let (_temp_dir, traverser) = setup_test_db();

        let callees = traverser.get_callees("func_a", 1).unwrap();
        assert_eq!(callees.len(), 1);
        assert_eq!(callees[0].0.id, "func_b");
    }

    #[test]
    fn test_find_path() {
        let (_temp_dir, traverser) = setup_test_db();

        let path = traverser.find_path("func_a", "func_c", vec![EdgeKind::Calls]).unwrap();
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(path.len(), 2); // func_a -> func_b -> func_c
    }

    #[test]
    fn test_impact_radius() {
        let (_temp_dir, traverser) = setup_test_db();

        let impact = traverser.get_impact_radius("func_c", 2).unwrap();
        // func_c is called by func_b, which is called by func_a
        assert!(impact.nodes.len() >= 2);
    }
}
