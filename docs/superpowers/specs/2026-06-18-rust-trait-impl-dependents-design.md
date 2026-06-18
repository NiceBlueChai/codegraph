# Rust trait implementation dependents parity

## Context

The TypeScript implementation extracts Rust traits and trait method declarations, records
`impl Trait for Type` as an `implements` dependency, and bridges trait methods to same-name implementation methods.
That keeps modules that implement traits visible to `affected` and relationship queries.

## Scope

- Extract Rust `trait_item` as `trait` nodes.
- Extract trait `function_signature_item` declarations as contained method nodes.
- Extract impl-block methods as method nodes owned by the implementing type.
- Emit `implements` references from implementing types to traits.
- Synthesize trait-method to implementation-method `calls` edges after reference resolution.

## Non-goals

- Full Rust type inference.
- Generic trait bound resolution.
- Cross-crate trait implementation disambiguation.

## Acceptance

- `affected src/types.rs --filter src/consumer.rs` reports `src/consumer.rs` for `impl Render for Mine`.
