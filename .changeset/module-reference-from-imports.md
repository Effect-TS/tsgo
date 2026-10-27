---
"@effect/tsgo": patch
---

Quick fixes, refactors and completions now build references to Effect modules (`Effect`, `Layer`, `Schema`, `Data`, `Context`, ...) from the file's imports instead of reusing the receiver of the call being rewritten, and support namespace imports of the package barrel.

```ts
import * as X from "effect"

const program = source.pipe(X.Effect.map(() => undefined))
// effectMapVoid quick fix:
const program = source.pipe(X.Effect.asVoid)
```

When the call site reaches Effect through something other than an import, the fix now uses the import:

```ts
import { Effect } from "effect"
const E = Effect
const program = source.pipe(E.map(() => undefined))
// quick fix now produces Effect.asVoid instead of E.asVoid
```
