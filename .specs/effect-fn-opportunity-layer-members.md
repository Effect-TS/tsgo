# EffectFnOpportunity in Layer Member Functions

## Goal
Detect function-to-`Effect.fn` opportunities inside Layer service definitions, including object members provided through Layer combinators and members returned from `Layer.effect(...)(Effect.gen(...))`.

## Scope
- Applies to the `effectFnOpportunity` diagnostic rule.
- Covers function-valued service members defined in object literals used for Layer construction.
- Covers both direct object arguments and object literals returned from Layer factory effects.

## Requirements
1. The rule must consider function-valued object members as candidates when they are in a Layer service-definition context.
2. Layer context detection must support at least:
   - `Layer.succeed(Tag, { ... })`
   - curried forms like `Layer.succeed(Tag)({ ... })`
   - `Layer.effect(Tag)(Effect.gen(function* () { return { ... } }))`
3. For `Layer.effect(...)(Effect.gen(...))`, function-valued members inside the returned object literal must be eligible for `effectFnOpportunity` when they satisfy existing effect-returning eligibility checks.
4. Candidate detection should be based on semantic Layer context (type-aware where available), not only fragile syntax matching, so piped/composed call shapes are supported when they still produce a Layer service definition.
5. Suggested trace/inferred names should include service + member context when available. For example, in a `UserService` layer object member `getUser`, the inferred name should support `"UserService.getUser"`.
6. Existing safeguards for `effectFnOpportunity` (already-inside-Effect.fn checks, return-type constraints, unsupported function shapes) must remain in effect.

## Non-Goals
- Changing runtime behavior of Layer combinators.
- Rewriting non-Effect-returning functions.

## Acceptance Criteria
1. In `Layer.succeed(Database, { query: (...) => Effect... })`, `query` is detected as an `effectFnOpportunity` candidate when eligible.
2. In `Layer.succeed(Database)({ query: (...) => Effect... })`, `query` is detected as an `effectFnOpportunity` candidate when eligible.
3. In `Layer.effect(UserService)(Effect.gen(function*(){ return { getUser: (...) => Effect.gen(...) } }))`, `getUser` is detected as an `effectFnOpportunity` candidate when eligible.
4. For the `UserService.getUser` case above, suggested conversion supports `Effect.fn("UserService.getUser")(... )` naming behavior.
5. Cases outside Layer service-definition context do not gain new false-positive opportunities.
