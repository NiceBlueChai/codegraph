# Rust Go top-level var closure callers parity

## Context

The TypeScript implementation extracts Go `var_spec` declarations as variable nodes and walks their initializer with
that variable on the scope stack. Calls inside a top-level closure such as a Cobra `RunE: func(){ Wire() }` are therefore
attributed to the variable (`rootCmd`) instead of the file node.

## Scope

- Extract simple top-level Go `var name = initializer` declarations as variable nodes.
- Walk the initializer for calls and named composite literals using the variable node as the source.
- Exclude variable initializer spans from the fallback file-level composite scan to avoid duplicate file-scope edges.

## Non-goals

- Full Go declaration grammar.
- Multi-name `var (...)` groups.
- Precise anonymous function nodes.

## Acceptance

- `callers Wire --json` reports a `variable` named `rootCmd`.
- `callers Wire --json` does not report a file node for the closure call.
