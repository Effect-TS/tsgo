// Extends parity coverage of layerGraphFollowSymbols to depth 2 across 3 files
// (base.ts → middle.ts → deep.ts). There is no direct .repos equivalent for the
// deep (depth-2) variant; see layerGraphFollowSymbols for depth-1 parity
// verification against .repos/effect-language-service reference snapshots.
// The same three documented divergences apply:
//   1. Alphabetical sorting of provides/requires type lists (extract.go)
//   2. Cross-file location annotations in flat output format (format.go)
//   3. "at in" double preposition fix in quickinfo for cross-file refs (format.go)
// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "layerGraphFollowDepth": 2
      }
    ]
  }
}
// @filename: base.ts
import { Effect, Layer, ServiceMap } from "effect"

export class Database extends ServiceMap.Service<Database>()("Database", {
  make: Effect.succeed({})
}) {
  static Default = Layer.effect(this, this.make)
}
export class FileSystem extends ServiceMap.Service<FileSystem>()("FileSystem", {
  make: Effect.succeed({})
}) {
  static Default = Layer.effect(this, this.make)
}
export class Cache extends ServiceMap.Service<Cache>()("Cache", {
  make: Effect.as(FileSystem.asEffect(), {})
}) {
  static Default = Layer.effect(this, this.make)
}

export const cacheWithFs = Cache.Default.pipe(Layer.provide(FileSystem.Default))

export const baseAlias = cacheWithFs
// @filename: middle.ts
import { cacheWithFs, baseAlias } from "./base"

export const wrappedCache = cacheWithFs

export const wrappedAlias = baseAlias
// @filename: deep.ts
import { wrappedCache, wrappedAlias } from "./middle"

export const twoLevel = wrappedCache

export const threeLevel = wrappedAlias
