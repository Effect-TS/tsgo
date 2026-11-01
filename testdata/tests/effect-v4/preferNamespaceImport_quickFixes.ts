// @filename: package.json
{ "name": "@effect/test-app", "version": "1.0.0" }

// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect", "@acme/kit"],
        "importAliases": { "@acme/kit": "Kit" }
      }
    ]
  }
}

// @filename: node_modules/@acme/kit/package.json
{ "name": "@acme/kit", "version": "1.0.0", "types": "./dist/index.d.ts", "exports": { ".": { "types": "./dist/index.d.ts" }, "./*": { "types": "./dist/*.d.ts" } } }

// @filename: node_modules/@acme/kit/dist/index.d.ts
export * as Dialog from "./Dialog.js"

// @filename: node_modules/@acme/kit/dist/Dialog.d.ts
export declare const open: () => void

// @filename: uses.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning

// fix: the named import becomes a member of Array, shorthand properties included
import { map } from "effect/Array"

interface User {
  readonly name: string
}
export const names = (users: ReadonlyArray<User>) => map(users, (user) => user.name)
export const helpers = { map }

// @filename: siblings.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning

// fix: the reported specifier leaves the declaration, and the others stay
import { map, filter } from "effect/Array"

export const evens = filter(map([1, 2, 3], (n) => n * 2), (n) => n > 2)

// @filename: reuse.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning

// fix: the uses are qualified with the namespace import already in the file
import * as Option from "effect/Option"
import { some } from "effect/Option"

export const values = [some(1), Option.none()]

// @filename: style.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning

// fix: quotes and semicolons are kept
import { map } from 'effect/Array';

export const doubled = map([1, 2], (n) => n * 2);

// @filename: alias.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning

// fix: importAliases names the namespace
import { open } from "@acme/kit/Dialog"

export const show = () => open()
