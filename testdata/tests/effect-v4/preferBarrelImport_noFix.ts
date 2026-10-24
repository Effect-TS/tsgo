// @filename: package.json
{ "name": "@effect/test-app", "version": "1.0.0" }

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

// @filename: globalValue.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferBarrelImport:warning

// no fix: Array would rebind the global Array that the file reads
import { map } from "effect/Array"

export const isList = Array.isArray
export const doubled = map([1, 2], (n) => n * 2)

// @filename: globalType.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferBarrelImport:warning

// no fix: Record would rebind the global Record type that the file reads
import { empty } from "effect/Record"

export type Lookup = Record<string, number>
export const none = empty()

// @filename: bound.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferBarrelImport:warning

// no fix: the file already binds Option
import { some } from "effect/Option"

const Option = { label: "local" }
export const values = [some(1), Option]

// @filename: shadowed.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferBarrelImport:warning

// no fix: an inner scope binds Option
import { some } from "effect/Option"

export const wrap = (Option: number) => some(Option)

// @filename: reexport.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferBarrelImport:warning

// no fix: the binding is re-exported, and Option.isSome can't stand in an export specifier
import { isSome } from "effect/Option"

export { isSome }

// @filename: comment.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferBarrelImport:warning

// no fix: the rewrite would drop the comment inside the declaration
import { /* the empty Option */ none } from "effect/Option"

export const nothing = none()
