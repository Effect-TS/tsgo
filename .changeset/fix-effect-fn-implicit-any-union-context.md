---
"@effect/tsgo": patch
---

Fix `effectFnImplicitAny` so contextual union types suppress the diagnostic when any union member provides a callable contextual type.

This aligns nested `Effect.fnUntraced` callbacks in union-typed APIs with TypeScript's `noImplicitAny` behavior.
