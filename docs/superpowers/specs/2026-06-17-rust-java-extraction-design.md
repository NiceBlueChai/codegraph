# Rust Java extraction parity

## Context

The TypeScript implementation has a Java tree-sitter extractor for classes, methods, imports, and calls. The Rust
implementation currently detects `.java` files but does not route them through any extractor, so query commands cannot
build Java call relationships.

## Scope

- Reuse the existing C-family fallback scanner for Java method declarations and call references.
- Cover direct calls and `this.Method()` calls for same-file graph resolution.
- Add a CLI parity test for `callers`, `callees`, and `impact`.

## Non-goals

- Full Java package/import resolution.
- Java-specific class/interface/enum extraction in this slice.
- Adding a new `tree-sitter-java` dependency without approval.

## Acceptance

- A Java fixture with `Run()` calling `Helper()` reports `Run` from `callers Helper`.
- The same fixture reports `Helper` from `callees Run`.
- `impact Helper` includes `Run` and a positive edge count.
- A Java `this.Clean()` method call resolves to `Clean`.
