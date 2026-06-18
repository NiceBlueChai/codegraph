# Rust struct literal dependents parity

## Context

The TypeScript implementation treats Rust struct expressions such as `Widget { n: 1 }` as instantiations. Without that
edge, a module that only constructs a struct can fail to appear as a dependent of the defining module.

## Scope

- Extract Rust `struct_expression` nodes from the tree-sitter parser.
- Emit an instantiation reference to the named struct type.
- Reuse existing Rust `use` path and exact-name resolution.

## Non-goals

- Full Rust type inference.
- Macro-expanded struct literals.
- Generic argument handling beyond trimming text from the parsed type node.

## Acceptance

- `affected src/types.rs --filter src/consumer.rs` reports `src/consumer.rs` for `Widget { n: 1 }`.
