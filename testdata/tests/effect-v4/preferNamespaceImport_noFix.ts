// @filename: package.json
{ "name": "@effect/test-app", "version": "1.0.0" }

// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect", "@acme/kit"],
        "topLevelNamedReexports": "follow"
      }
    ]
  }
}

// @filename: node_modules/@acme/kit/package.json
{ "name": "@acme/kit", "version": "1.0.0", "types": "./dist/index.d.ts", "exports": { ".": { "types": "./dist/index.d.ts" }, "./*": { "types": "./dist/*.d.ts" } } }

// @filename: node_modules/@acme/kit/dist/index.d.ts
export { deep } from "./nested/deep.js"

// @filename: node_modules/@acme/kit/dist/nested/deep.d.ts
export declare const deep: number

// @filename: globalValue.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning

// no fix: Array would rebind the global Array that the file reads
import { map } from "effect/Array"

export const isList = Array.isArray
export const doubled = map([1, 2], (n) => n * 2)

// @filename: followGlobal.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning

// no fix: pipe would become Function.pipe, and Function would rebind the global the file reads
import { pipe } from "effect"

export const isFunction = (u: unknown): u is Function => typeof u === "function"
export const value = pipe(1, (n) => n + 1)

// @filename: reexport.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning

// no fix: the binding is re-exported, and Array.map can't stand in an export specifier
import { map } from "effect/Array"

export { map }

// @filename: comment.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning

// no fix: the rewrite would drop the comment inside the declaration
import { map /* the Effect one */ } from "effect/Array"

export const doubled = map([1, 2], (n) => n * 2)

// @filename: noTarget.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferNamespaceImport:warning

// no fix: the report names no module to import as a namespace
import { deep } from "@acme/kit"

export const value = deep
