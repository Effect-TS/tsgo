// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}

// @filename: barrel.ts
// @effect-diagnostics *:off
export { pipe } from "effect/Function"

// @filename: /node_modules/cache-wrapper/package.json
{ "name": "cache-wrapper", "version": "1.0.0", "type": "module", "exports": { ".": "./index.d.ts" } }

// @filename: /node_modules/cache-wrapper/index.d.ts
export { pipe } from "effect/Function"

// @filename: /node_modules/cache-wrapper/node_modules/effect/package.json
{ "name": "effect", "version": "0.0.0-cache-test", "type": "module", "exports": { "./Function": "./Function.d.ts" } }

// @filename: /node_modules/cache-wrapper/node_modules/effect/Function.d.ts
export declare function pipe<A>(value: A): A

// @filename: test.ts
// @effect-diagnostics *:off
// @effect-diagnostics unnecessaryPipe:warning
import * as RootFunction from "./barrel.js"
import * as NestedFunction from "cache-wrapper"

export const rootOne = RootFunction.pipe(1)
export const rootTwo = RootFunction.pipe(2)
export const nestedOne = NestedFunction.pipe(3)
export const nestedTwo = NestedFunction.pipe(4)
