# Rust C# record dependents parity

## Context

The TypeScript implementation indexes C# `record` and `record struct` declarations as type nodes. It also resolves
references and instantiations such as `IEnumerable<Box>` and `new Box(...)`, so record definition files participate in
`affected`.

## Scope

- Extract simple C# `record`, `record class`, and `record struct` declarations.
- Emit C# type/instantiation references for simple record usages.
- Reuse the existing resolver exact-name strategy.

## Non-goals

- Full C# grammar parsing.
- Namespace-qualified type resolution.
- Complete generic type walking.

## Acceptance

- `affected types.cs` reports `use.cs` when `use.cs` references and instantiates `Box`.
