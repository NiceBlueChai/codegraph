# Rust TS import dependents parity

## Context

The TypeScript implementation emits import references for TS/JS imports so file-level dependents and `affected` can see
dependencies even when imported values are not called. Rust extracts import references but does not resolve relative
module specifiers like `./foo` to `src/foo.ts` file nodes.

## Scope

- Resolve relative TS/JS import specifiers to indexed file nodes.
- Support extensionless imports and `index.*` module files.
- Preserve the existing resolver pipeline and edge direction: importer file node -> imported file node.

## Non-goals

- Path aliases such as `@/foo` or `$lib/foo`.
- Package imports from `node_modules`.
- Full TS import binding resolution in this slice.

## Acceptance

- A test file importing `../src/foo` is reported by `codegraph affected src/foo.ts --json`.
