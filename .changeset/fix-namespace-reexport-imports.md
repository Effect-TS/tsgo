---
"@effect/tsgo": patch
---

Fix `namespaceImportPackages` completions for namespace reexports such as `Effect`, generating `import * as Effect from "effect/Effect"` instead of `import { Effect } from "effect"`.
