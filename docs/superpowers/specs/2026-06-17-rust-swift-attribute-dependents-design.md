# Rust Swift attribute dependents parity

## Context

The TypeScript implementation links Swift attributes such as `@Argument` to the wrapper type. This makes a
`@propertyWrapper` declaration visible to `affected` when stored properties use it.

## Scope

- Extract simple Swift `struct`, `class`, and `enum` declarations as type nodes in the fallback parser.
- Emit `decorates` references for Swift attributes such as `@Argument`.
- Reuse the existing `decorates` edge mapping.

## Non-goals

- Full Swift grammar parsing.
- Attribute argument parsing.
- Swift module import resolution.

## Acceptance

- `affected Sources/M/Wrap.swift` reports `Sources/M/Cmd.swift` when `Cmd.swift` uses `@Argument`.
