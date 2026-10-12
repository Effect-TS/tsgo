---
"@effect/tsgo": patch
---

Fix a crash in the "Convert to Effect.fn" refactor when `gen` is a named import or a local alias.

```ts
import { gen } from "effect/Effect"
export const foo = () => gen(function*() { return 1 })
```

Requesting code actions on `gen` used to fail the whole `textDocument/codeAction` request with a nil pointer panic. The refactor is now skipped for these forms, which matches the upstream language service. The other refactors are still returned.
