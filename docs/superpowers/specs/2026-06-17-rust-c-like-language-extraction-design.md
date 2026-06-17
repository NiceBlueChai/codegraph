# Rust C-like language extraction parity

## Context

The TypeScript implementation has dedicated extractors for PHP, Swift, Kotlin, and Dart. The Rust implementation already
detects their file extensions but does not parse them, so relationship commands return no basic call graph.

## Scope

- Reuse the existing C-family fallback scanner for simple braced functions and methods in PHP, Swift, Kotlin, and Dart.
- Cover direct calls and receiver calls normalized by the existing fallback (`this.Method`, `self.Method`, `$this->Method`).
- Add one CLI parity test covering `callers`, `callees`, and `impact` for these languages.

## Non-goals

- Full language-specific syntax coverage.
- Import/package resolution and framework routing.
- Adding grammar dependencies without approval.

## Acceptance

- Each covered language fixture reports `Run` from `callers Helper`.
- Each covered language fixture reports `Helper` from `callees Run`.
- Each covered language fixture reports `Run` from `impact Helper`.
- Receiver-style method calls resolve to `Clean`.
