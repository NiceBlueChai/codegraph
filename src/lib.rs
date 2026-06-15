pub mod types;
pub mod db;
pub mod core;
pub mod extraction;
pub mod sync;
pub mod mcp;

// Re-export main types
pub use types::*;
pub use db::{DatabaseConnection, QueryBuilder};

/// CodeGraph library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
