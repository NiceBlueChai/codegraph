# Rust Go pointer conversion dependents parity

## Context

The TypeScript implementation normalizes Go parenthesized type conversions such as `(*Wrapped)(x)` and `(Wrapped)(x)`
from the raw callee text to `Wrapped`. That keeps the converted-to type visible to `affected`.

## Scope

- Recognize parenthesized Go type conversion calls inside function bodies.
- Emit a normalized reference to the converted-to type name.
- Reuse existing Go type extraction and resolver behavior.

## Non-goals

- Full Go expression parsing.
- Distinguishing every parenthesized callable expression from a type conversion.
- Go package import disambiguation.

## Acceptance

- `affected types.go --filter use.go` reports `use.go` for `_ = (*Wrapped)(x)`.
