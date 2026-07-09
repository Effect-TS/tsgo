# MissingEffectError Return Awareness

## Goal
Prevent false-positive Effect error diagnostics when a generator branch already returns the failing Effect expression.

## Scope
- Applies to diagnostics emitted by the `missingEffectError` rule in generator-based Effect flows.
- Covers branches that use `return yield* ...` with `Effect.fail(...)`.

## Requirements
1. A branch that explicitly returns a failing effect via `return yield* Effect.fail(...)` must not trigger a diagnostic claiming a missing return or missing return wrapping for that branch.
2. Equivalent successful branches (for example `return yield* Effect.succeed(...)`) must continue to type-check/diagnose consistently with existing rule behavior.
3. The rule must avoid suggesting corrective edits that duplicate an already-present `return`.

## Non-Goals
- Redefining broader TypeScript control-flow diagnostics.
- Changing unrelated Effect diagnostics.

## Acceptance Criteria
1. For the reported `Effect.eventually(Effect.gen(...))` pattern where the failure branch already uses `return yield* Effect.fail("...")`, no false-positive diagnostic is emitted for “missing return” on that failing branch.
2. Existing valid diagnostics for genuinely missing returns remain unaffected.
