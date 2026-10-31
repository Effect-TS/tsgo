// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect"]
      }
    ]
  }
}

// @filename: namespace.ts
// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning

// fix: the binding is already the namespace, so only the import changes
import { Effect } from "effect"

export const program = Effect.succeed(1)

// @filename: uses.ts
// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning

// fix: the named import becomes a member of Array, shorthand properties included
import { map } from "effect/Array"

export const doubled = map([1, 2], (n) => n * 2)
export const helpers = { map }

// @filename: globalValue.ts
// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning

// no fix: Array would rebind the global Array that the file reads
import { map } from "effect/Array"

export const isList = Array.isArray
export const doubled = map([1, 2], (n) => n * 2)
