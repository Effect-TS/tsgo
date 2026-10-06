---
"@effect/tsgo": minor
---

Add the opt-in `apiStabilityLeak` diagnostic for Effect v4 package maintainers.
It reports exports whose public types, parameters, or return values expose a
less stable API: stable exports must not expose unstable or experimental types,
and unstable exports must not expose experimental types. Untagged exports are
treated as stable; experimental exports impose no restriction.

For example, `export interface StableApi { value: ExperimentalType }` reports
a leak when `ExperimentalType` has an `@stability experimental` tag. Public
members, generic arguments, inferred types, and re-exports are checked without
recursively instantiating types.

The rule is off by default and excluded from the recommended, strict, and
correctness presets. Enable the new `maintainers` preset to report it as a
warning, or configure `apiStabilityLeak` directly. Declared stability lookups are
shared with the existing unstable and experimental API usage diagnostics.
