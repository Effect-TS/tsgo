---
"@effect/tsgo": minor
---

Add `preferNamespaceImport` and `preferBarrelImport`, two diagnostics that report existing imports that contradict the import style configured with `namespaceImportPackages` and `barrelImportPackages`. Auto-import already follows these options; the new rules also cover imports written by hand. Both are off by default and have no quick fix.

`preferNamespaceImport` reports named imports from packages in `namespaceImportPackages`:

```ts
// "namespaceImportPackages": ["effect"]
import { Effect } from "effect" // expected: import * as Effect from "effect/Effect"
import { map } from "effect/Array" // expected: import * as Array from "effect/Array"
import { pipe } from "effect" // reported only with "topLevelNamedReexports": "follow"
```

`preferBarrelImport` reports imports of individual modules from packages in `barrelImportPackages`:

```ts
// "barrelImportPackages": ["effect"]
import * as Record from "effect/Record" // expected: import { Record } from "effect"
import * as HttpClient from "effect/http/HttpClient" // expected: import { HttpClient } from "effect/http"
```

Both the rules and auto-import find a barrel from the package files, for any package, instead of from a built-in list of Effect paths. A module's barrel is the nearest `index` module above it that forwards it with `export * as Name`, and `Name` is the import name used. Barrel files are read from disk when no file in the program imports them yet. Auto-import with `barrelImportPackages` uses the same lookup, so it no longer rewrites a module that no barrel forwards, and no longer rewrites an import that already comes from a barrel (such as `pipe` from `"effect"`) into `import { effect } from "effect"`.
