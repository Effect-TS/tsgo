---
"@effect/tsgo": minor
---

Add the `runOfExitToRunExit` diagnostic, which replaces `Effect.runPromise` applied to `Effect.exit` with the dedicated `Effect.runPromiseExit` runner.
