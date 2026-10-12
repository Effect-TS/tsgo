---
"@effect/tsgo": patch
---

Fix a crash in the "Convert to Effect.fn" refactor when `gen` is a named import or a local alias.

```ts
import { gen } from "effect/Effect"
export const foo = () => gen(function*() { return 1 })
```

Requesting code actions on `gen` used to fail the whole `textDocument/codeAction` request with a nil pointer panic. The refactor now rewrites to `Effect.fn`, using the file's existing Effect import name when there is one:

```ts
import { gen } from "effect/Effect"
export const foo = Effect.fn("foo")(function* () { return 1; });
```

The refactor does not add a missing `Effect` import. The user adds it.
