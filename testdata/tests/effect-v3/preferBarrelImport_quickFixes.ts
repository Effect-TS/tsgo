// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "barrelImportPackages": ["effect"]
      }
    ]
  }
}

// @filename: namespace.ts
// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics preferBarrelImport:warning

// fix: the namespace import keeps its binding
import * as O from "effect/Option"

export const value = O.some(1)

// @filename: uses.ts
// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics preferBarrelImport:warning

// fix: the named imports become members of Record, shorthand properties included
import { empty, set } from "effect/Record"

export const counts = set(empty<string, number>(), "a", 1)
export const factories = { empty }

// @filename: globalValue.ts
// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics preferBarrelImport:warning

// no fix: Array would rebind the global Array that the file reads
import { map } from "effect/Array"

export const isList = Array.isArray
export const doubled = map([1, 2], (n) => n * 2)
