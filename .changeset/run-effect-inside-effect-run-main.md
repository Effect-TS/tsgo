---
"@effect/tsgo": minor
---

`runEffectInsideEffect` now reports `runMain` from `@effect/platform-node`, `@effect/platform-bun` and `@effect/platform-browser` when it is called inside an Effect, such as an `Effect.gen` body or an `Effect.sync` thunk. That call starts a second root runtime, so the outer program cannot interrupt it, see its failures, or close its scope. The diagnostic has no quick fix: return the Effect and call `runMain` once at the application entry point.

```ts
import { NodeRuntime } from "@effect/platform-node"
import { Effect } from "effect"

const serve = Effect.log("serving")

// reported
export const handler = Effect.sync(() => {
  NodeRuntime.runMain(serve)
})

// not reported
NodeRuntime.runMain(serve)
```
