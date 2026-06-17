# Rust TS re-export dependents parity

## Context

The TypeScript implementation emits import references for `export { X } from './module'` so barrel files and re-export
hubs are visible to file dependents and `affected`. Rust currently handles `import_statement` but ignores
`export_statement` nodes with a source module.

## Scope

- Treat TS/JS `export_statement` nodes with a `source` field like import references.
- Reuse the relative import path resolution added for TS/JS imports.
- Add a CLI parity test that traverses `foo.ts -> bar.ts -> foo.test.ts`.

## Non-goals

- Symbol-level alias matching for re-exported names.
- `export * from` expansion beyond file-level dependency.

## Acceptance

- `codegraph affected src/foo.ts --json` includes a test importing a re-exporting `src/bar.ts`.
