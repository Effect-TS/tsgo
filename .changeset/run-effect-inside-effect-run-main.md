---
"@effect/tsgo": minor
---

`runEffectInsideEffect` now also reports `runMain` from the platform runtime modules (`NodeRuntime`, `BunRuntime` and `BrowserRuntime`) when it is called inside an Effect context: an `Effect.gen` or `Effect.fn` generator (including the callbacks nested in it), or an Effect constructor thunk such as `Effect.sync`. Calling `runMain` there starts a second root runtime that the surrounding program does not own: interruption does not reach it, its failures do not propagate, and it installs its own signal handlers. `runMain` is resolved through type information, so aliased and deep imports are matched too. There is no quick fix, because the fix moves the call to the entrypoint.

```ts
import { NodeRuntime } from "@effect/platform-node"
import { Effect } from "effect"

const serve = Effect.log("serving")

export const handler = Effect.sync(() => {
  NodeRuntime.runMain(serve)
  // ^- `NodeRuntime.runMain` is called inside an Effect context and starts a separate root runtime, ...
})

// Instead, return the Effect and run it once, at the entrypoint
NodeRuntime.runMain(serve)
```
