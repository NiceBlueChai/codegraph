# Rust Go extraction parity

## Context

The Rust parser already recognizes `.go` files as `Language::Go`, but the fallback extractor does not emit Go functions,
methods, or call references. Query commands such as `callers`, `callees`, and `impact` therefore return empty results for
basic Go projects even when the source has direct calls.

## Scope

- Extract top-level Go functions declared with `func Name(...)`.
- Extract Go methods declared with `func (receiver Type) Name(...)`.
- Extract simple call references inside function and method bodies.
- Normalize selector calls such as `pkg.Helper()` and `receiver.Handle()` to their tail symbol for same-package graph
  resolution.

## Non-goals

- Full Go import/package resolution.
- Interface satisfaction, generated-code bridges, and framework routing.
- Replacing the fallback extractor with tree-sitter Go in this slice.

## Acceptance

- A Go fixture with `Run()` calling `Helper()` reports `Run` from `callers Helper`.
- The same fixture reports `Helper` from `callees Run`.
- `impact Helper` includes `Run` and a positive edge count.
