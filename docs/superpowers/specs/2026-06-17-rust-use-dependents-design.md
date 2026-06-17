# Rust use dependents parity

## Context

The TypeScript implementation emits import references for Rust `use` and `pub use` declarations. This lets re-export hubs
and module imports participate in `affected`. The Rust implementation parses Rust with tree-sitter but does not extract
`use_declaration` refs.

## Scope

- Extract unresolved import refs from Rust `use_declaration` text.
- Resolve `self::module::Item` and `crate::module::Item` to indexed module files.
- Preserve module-path disambiguation when leaf names collide.

## Non-goals

- Full Rust `use` grammar coverage.
- Wildcard import expansion.
- Symbol-level module privacy semantics.

## Acceptance

- `affected src/api/widget.rs` includes `src/api/mod.rs` for `pub use self::widget::Widget`.
- `pub use crate::fast::read` depends on `src/fast.rs`, not `src/slow.rs`.
