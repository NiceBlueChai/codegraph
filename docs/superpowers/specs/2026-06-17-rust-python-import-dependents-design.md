# Rust Python import dependents parity

## Context

The TypeScript implementation emits Python import references for `from module import name` and module imports such as
`from . import certs`, so imported-but-not-called values still affect dependents. The Rust tree-sitter parser currently
does not extract Python import-from dependencies into useful references.

## Scope

- Extract import references from Python `import_from_statement` nodes.
- Resolve Python same-directory and relative module import refs to indexed `.py` file nodes.
- Keep symbol imports resolving through the existing exact-name resolver.

## Non-goals

- Full Python package search path modeling.
- Wildcard import expansion.
- Third-party package resolution.

## Acceptance

- `affected` reports a test file that imports a Python function without calling it.
- `affected` reports a test file that uses `from . import certs` as a module dependency.
