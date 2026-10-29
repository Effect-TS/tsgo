---
"@effect/tsgo": minor
---

`runEffectInsideEffect` now reports `runMain` from `@effect/platform-node`, `@effect/platform-bun` and `@effect/platform-browser` when it is called inside an Effect, such as an `Effect.gen` body or an `Effect.sync` thunk. That call starts a second root runtime, so the outer program cannot interrupt it, see its failures, or close its scope. The `runMain` diagnostic has no quick fix: return the Effect and call `runMain` once at the application entry point.

The rule now also reports runners in pipe position, for both `Effect.run*` and `runMain`. The quick fix for `Effect.run*` works in pipe position too, and keeps optional arguments such as `{ signal }`. In Effect v3 it now writes the curried `Runtime.runPromise(effectRuntime)(effect)` form.

```ts
import { NodeRuntime } from "@effect/platform-node"
import { Effect } from "effect"

const serve = Effect.log("serving")

// reported
export const handler = Effect.sync(() => {
  NodeRuntime.runMain(serve)
})

export const piped = Effect.gen(function*() {
  yield* Effect.log("starting")
  const run = () => serve.pipe(Effect.runFork)
  // quick fix: serve.pipe(Effect.runForkWith(effectContext))
  return run
})

// not reported
NodeRuntime.runMain(serve)
```
