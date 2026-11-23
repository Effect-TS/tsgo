---
"@effect/tsgo": patch
---

`missedPipeableOpportunity` no longer suggests a pipe that starts at a type predicate or assertion function applied to a narrowable reference. TypeScript narrows only on the direct call, so the pipe form loses the narrowing.

```ts
declare const result: Result.Result<number, string>

assert(Result.isFailure(result)) // no longer reported
result.failure // still narrowed

// The suggested `result.pipe(Result.isFailure, assert)` would leave `result`
// un-narrowed, and `result.failure` would fail with TS2339.
```

Predicates applied to a call result, such as `assert(Option.isSome(Option.map(o, f)))`, are still reported. A predicate call on a reference is skipped even when its result is not used for narrowing, as in `String(Result.isFailure(result))`.
