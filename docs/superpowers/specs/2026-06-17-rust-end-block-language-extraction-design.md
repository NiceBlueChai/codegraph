# Rust end-block language extraction parity

## Context

The TypeScript implementation has tree-sitter extractors for Ruby, Lua, and Luau. The Rust implementation detects these
extensions but does not extract functions or calls, so relationship commands return empty results.

## Scope

- Add a small fallback extractor for simple Ruby `def Name ... end` methods.
- Add a small fallback extractor for simple Lua/Luau `function Name(...) ... end` functions.
- Normalize receiver calls such as `self.Clean()`, `Service.Clean()`, and `Service:Clean()` to the tail symbol.
- Add CLI parity coverage for `callers`, `callees`, and `impact`.

## Non-goals

- Nested block correctness.
- Module/import resolution.
- Full Ruby/Lua/Luau grammar support without tree-sitter.

## Acceptance

- Ruby, Lua, and Luau fixtures report `Run` from `callers Helper`.
- They report `Helper` from `callees Run`.
- `impact Helper` includes `Run` and a positive edge count.
- Receiver-style method calls resolve to `Clean`.
