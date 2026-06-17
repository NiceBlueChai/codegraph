# Rust Java annotation dependents parity

## Context

The TypeScript implementation treats Java `@interface` declarations as interface nodes and emits `decorates`
references for Java annotation usages. This makes annotation definition files visible to `affected`.

## Scope

- Extract Java `@interface Name` declarations as interface nodes.
- Emit `decorates` references for Java annotation usages such as `@MyAnno(...)`.
- Resolve `decorates` refs to `decorates` edges through the existing resolver.

## Non-goals

- Full Java grammar parsing.
- Annotation argument parsing.
- Java package-qualified annotation resolution.

## Acceptance

- `affected p/MyAnno.java` reports `p/User.java` when `User.java` uses `@MyAnno`.
