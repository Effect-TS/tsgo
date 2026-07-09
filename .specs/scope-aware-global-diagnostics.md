# Scope-Aware Global Diagnostics

## Goal

Port the reference implementation's scope-aware global API diagnostics into the Go language service so users receive different guidance for global API usage inside Effect-yielding scopes versus outside them.

## Scope

- Covers the global API diagnostics for `fetch`, `console`, `Date`, `Math.random`, `setTimeout`, and `setInterval`.
- Covers both outside-Effect and inside-Effect rule variants where the reference implementation now splits behavior by scope.
- Covers shared symbol normalization needed to reliably recognize global APIs through aliases and simple indirections.
- Covers diagnostic message wiring, rule registration, metadata exposure, tests, and README updates for the new and renamed diagnostics.

## Requirements

1. The Go implementation must match the final reference behavior after `Effect-TS/language-service` commit `5814a58`, not the intermediate unsplit behavior from `78e78d5`.
2. The rule set must expose separate diagnostics for global API usage outside Effect-yielding scopes and inside Effect-yielding scopes whenever the reference implementation does so.
3. The following diagnostic names must be supported:
   - `globalFetch`
   - `globalFetchInEffect`
   - `globalConsole`
   - `globalConsoleInEffect`
   - `globalDate`
   - `globalDateInEffect`
   - `globalRandom`
   - `globalRandomInEffect`
   - `globalTimers`
   - `globalTimersInEffect`
4. Scope classification must reuse the existing Go Effect context analysis rather than introducing a parallel scope-detection system.
5. Outside-Effect diagnostics must report only matches that are not inside an Effect-yielding scope.
6. In-Effect diagnostics must report only matches that are inside an Effect-yielding scope.
7. `globalFetch` and `globalFetchInEffect` must preserve version-aware guidance:
   - use `@effect/platform` messaging for Effect v3
   - use `effect/unstable/http` messaging for Effect v4
8. The implementation must add a shared symbol-normalization helper so diagnostics can recognize the underlying global symbol even when the code uses:
   - direct global references
   - aliased references
   - simple variable indirections that still resolve to the same global value
9. The symbol-normalization behavior should be shared across the global diagnostics rather than copied into each rule.
10. `globalConsole` and `globalConsoleInEffect` must cover the console methods supported by the reference implementation: `log`, `warn`, `error`, `info`, `debug`, and `trace`.
11. `globalDate` and `globalDateInEffect` must distinguish the reference implementation's supported forms:
   - `Date.now()` guidance
   - `new Date()` guidance
12. `globalRandom` and `globalRandomInEffect` must cover `Math.random()`.
13. `globalTimers` and `globalTimersInEffect` must cover `setTimeout` and `setInterval`, with diagnostic guidance matching the reference implementation's preferred Effect alternatives.
14. The diagnostic metadata and generated diagnostic messages must include all newly introduced rule names and message texts needed by the split rules.
15. The committed README diagnostics table must be updated to reflect the final scope-aware rule set and descriptions.
16. Test coverage must verify, at minimum:
   - outside-Effect matches
   - inside-Effect matches
   - no duplicate reporting between the split variants
   - alias or simple-indirection recognition for global APIs
   - v3/v4-specific `globalFetch` messaging

## Non-Goals

- Adding quick fixes for these diagnostics.
- Changing the existing Effect context analysis model beyond what is needed to consume it.
- Porting unrelated diagnostics that landed after these commits.

## Acceptance Criteria

1. A project using a supported global API outside an Effect-yielding scope receives only the outside-Effect diagnostic variant.
2. A project using the same API inside an Effect-yielding scope receives only the corresponding `InEffect` diagnostic variant.
3. `globalFetch` messaging changes based on detected supported Effect version exactly as in the reference implementation.
4. The Go diagnostics can still recognize targeted global APIs when code introduces a simple alias or variable indirection.
5. The diagnostics table in `README.md` lists the full final scope-aware rule set and descriptions.
6. Repository validation and baseline coverage confirm the new rules are registered and emitted with the expected names and messages.
