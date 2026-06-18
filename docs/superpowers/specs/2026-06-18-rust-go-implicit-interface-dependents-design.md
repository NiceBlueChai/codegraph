# Rust Go implicit interface dependents parity

## Context

The TypeScript implementation synthesizes Go implicit interface satisfaction. Go has no `implements` keyword, so a
struct whose method set covers an interface must be linked structurally. It then bridges interface methods to matching
implementation methods with `calls` edges so impact/affected can reach concrete implementations.

## Scope

- Extract simple Go interface method specs as method nodes contained by the interface node.
- Link Go receiver methods to same-package receiver types with `contains` edges.
- Synthesize same-package interface method to implementation method `calls` edges by method-name set coverage.

## Non-goals

- Signature-level method matching.
- Embedded interfaces.
- Cross-package interface satisfaction.
- Generated gRPC stub bridging.

## Acceptance

- `affected codec/json.go --filter codec/api.go` reports `codec/api.go` when `jsonApi` satisfies `Core`.
