---
"@effect/tsgo": patch
---

`catchAllToMapError` and `catchToOrElseSucceed` now name the catch variant the callee resolves to in their message. Previously a named import, renamed import or local alias of `Effect.catchAll` was reported as `Effect.catch`:

```ts
import { catchAll } from "effect/Effect"

pipe(program, catchAll((error) => fail(error.length)))
// before: ... more directly than `Effect.catch` followed by `Effect.fail`
// after:  ... more directly than `Effect.catchAll` followed by `Effect.fail`
```
