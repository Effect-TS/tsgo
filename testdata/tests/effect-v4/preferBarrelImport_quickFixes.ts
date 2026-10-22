// @filename: package.json
{ "name": "@effect/test-app", "version": "1.0.0" }

// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "barrelImportPackages": ["effect", "@acme/ui"],
        "importAliases": { "@acme/ui": "Ui" }
      }
    ]
  }
}

// @filename: node_modules/@acme/ui/package.json
{ "name": "@acme/ui", "version": "1.0.0", "types": "./dist/index.d.ts", "exports": { ".": { "types": "./dist/index.d.ts" }, "./*": { "types": "./dist/*.d.ts" } } }

// @filename: node_modules/@acme/ui/dist/index.d.ts
export * as Button from "./Button.js"

// @filename: node_modules/@acme/ui/dist/Button.d.ts
export declare const make: (label: string) => string

// @filename: uses.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferBarrelImport:warning

// fix: the named imports become members of Record, shorthand properties included
import { empty, set } from "effect/Record"

export const counts = set(empty<string, number>(), "a", 1)
export const factories = { empty }
export type Empty = typeof empty

// @filename: reuse.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferBarrelImport:warning

// fix: the uses are qualified with the Option import already in the file
import { Option } from "effect"
import { some, none } from "effect/Option"

export const values = [some(1), none(), Option.isSome]

// @filename: typeOnly.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferBarrelImport:warning

// fix: only types are imported, so the barrel import is type-only
import { type Duration } from "effect/Duration"

export type Timeout = Duration

// @filename: style.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferBarrelImport:warning

// fix: quotes and semicolons are kept
import { map } from 'effect/Array';

export const doubled = map([1, 2], (n) => n * 2);

// @filename: alias.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferBarrelImport:warning

// fix: importAliases names the binding
import { make } from "@acme/ui/Button"

export const save = make("save")
