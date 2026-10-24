---
"@effect/tsgo": minor
---

Add quick fixes to `preferBarrelImport` and `preferNamespaceImport`. A fix rewrites the reported import into the style the diagnostic asks for and, when the import binds names from the module, qualifies each use of them.

When the binding keeps its meaning, only the import changes:

```ts
// "barrelImportPackages": ["effect"]
import * as R from "effect/Record" // fix: import { Record as R } from "effect"

// "namespaceImportPackages": ["effect"]
import { Effect } from "effect" // fix: import * as Effect from "effect/Effect"
```

Named imports from a module become members of its namespace:

```ts
// "barrelImportPackages": ["effect"]
import { empty, set } from "effect/Record"
const counts = set(empty<string, number>(), "a", 1)

// after the fix
import { Record } from "effect"
const counts = Record.set(Record.empty<string, number>(), "a", 1)
```

The namespace gets the name auto-import would write, `importAliases` included. Shorthand properties are expanded (`{ empty }` becomes `{ empty: Record.empty }`), and an existing import of the namespace in the target style is reused.

The fix is not offered, and the diagnostic stays, when the namespace name is already bound in the file or names a global the file reads (`Array.isArray`, `Record<string, A>`, or `Function` for `pipe` with `topLevelNamedReexports: "follow"`), when the imported binding is re-exported, when the import declaration holds a comment, or when the report names no module to import.
