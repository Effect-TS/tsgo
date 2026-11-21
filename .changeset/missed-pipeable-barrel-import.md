---
"@effect/tsgo": patch
---

Fix `missedPipeableOpportunity` missing nested calls on modules imported by name from the `effect` barrel. Calls such as these are now reported just like their `import * as Effect from "effect/Effect"` equivalents:

```ts
import { Effect, Option } from "effect"

export const b1 = Effect.asVoid(Effect.flatten(e))
// -> e.pipe(Effect.flatten, Effect.asVoid)
export const b2 = Option.getOrUndefined(Option.flatten(oo))
// -> oo.pipe(Option.flatten, Option.getOrUndefined)
```
