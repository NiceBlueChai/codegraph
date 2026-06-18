# Rust module-path call dependents parity

## Context

The TypeScript implementation resolves Rust calls such as `users::router()` and
`database::profiles::find()` by treating the prefix as a Rust module path and the last segment as the target symbol.
This disambiguates common leaf names across sibling modules and keeps handler modules visible to `affected`.

## Scope

- Extract existing Rust `call_expression` references without changing their shape.
- Resolve Rust `A::B::leaf` references by mapping `A::B` to a module file.
- For bare module prefixes, try self-relative resolution before crate-relative resolution.
- For `crate`, `self`, and repeated `super` prefixes, anchor resolution the same way as the TypeScript implementation.
- Link only symbols in the resolved module file with supported target kinds.

## Non-goals

- Rocket `routes![...]` and `catchers![...]` macro token-tree extraction.
- Full Rust name resolution, external crates, or associated function inference.
- Rewriting Rust qualified names during extraction.

## Acceptance

- `affected src/http/users.rs --filter src/http/mod.rs` reports the parent module for `users::router()`.
- `affected src/database/profiles.rs --filter src/routes/mod.rs` reports the route module for
  `database::profiles::find()`, even when another `find` exists elsewhere.
