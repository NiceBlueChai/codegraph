# Rust Go composite literal dependents parity

## Context

The TypeScript implementation uses the Go tree-sitter grammar to treat named composite literals as instantiations:
`Widget{}` and `pkg.Widget{}` create references to the named type. That feeds `affected` for Go projects where a
struct definition is only used through a composite literal.

## Scope

- Extract simple Go `type Foo struct` and `type Foo interface` declarations.
- Emit Go instantiation references for named composite literals, including package-qualified `pkg.Foo{}`.
- Attribute function-body literals to the enclosing Go function, and top-level literals to the file node.
- Reuse the current resolver strategies and `Instantiates` edge kind.

## Non-goals

- Full Go tree-sitter migration.
- Go module/package import disambiguation.
- Slice, array, map, or anonymous struct literal resolution.

## Acceptance

- `affected render/xml.go --filter app.go` reports `app.go` for `render.XML{}` in a function.
- `affected render/xml.go --filter reg.go` reports `reg.go` for package-level registry literals.
