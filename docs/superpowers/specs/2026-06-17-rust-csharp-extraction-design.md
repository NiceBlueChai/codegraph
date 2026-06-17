# Rust C# extraction parity

## Context

The TypeScript implementation extracts C# classes, methods, properties, imports, and calls. The Rust implementation
detects `.cs` files but does not route them through an extractor, so relationship commands have no C# call graph.

## Scope

- Reuse the existing C-family fallback scanner for basic C# methods and calls.
- Cover direct calls and `this.Method()` calls.
- Add a CLI parity test for `callers`, `callees`, and `impact`.

## Non-goals

- Full C# namespace/import/property extraction.
- Conditional-compilation handling from the TypeScript extractor.
- Adding a new C# tree-sitter grammar dependency without approval.

## Acceptance

- A C# fixture with `Run()` calling `Helper()` reports `Run` from `callers Helper`.
- The same fixture reports `Helper` from `callees Run`.
- `impact Helper` includes `Run` and a positive edge count.
- A `this.Clean()` call resolves to `Clean`.
