// Parity with .repos/effect-language-service simple.ts snapshots verified for
// simplePipeIn and cacheWithFs at depth 0. Line numbers differ from .repos
// because the Go test's simple.ts omits the expect and liveWithPipeable exports;
// this is a test input difference, not a semantic divergence.
// Documented divergence that applies:
//   1. Alphabetical sorting of provides/requires type lists (extract.go)
// Divergences that do NOT apply at depth 0 (no cross-file symbol following):
//   2. Cross-file location annotations in flat output format (format.go)
//   3. "at in" double preposition fix in quickinfo for cross-file refs (format.go)
// The followSymbols.ts exports (followSymbols, moreComplex) at depth 0 have no
// direct .repos equivalent; depth-1 parity for these is verified via the
// layerGraphFollowSymbols test.
// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "layerGraphFollowDepth": 0
      }
    ]
  }
}
// @filename: simple.ts
import { Effect, Layer, ServiceMap } from "effect"

export class DbConnection extends ServiceMap.Service<DbConnection>()("DbConnection", {
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
export class UserRepository extends ServiceMap.Service<UserRepository>()("UserRepository", {
  make: Effect.as(Effect.andThen(DbConnection.asEffect(), Cache.asEffect()), {})
}) {
  static Default = Layer.effect(this, this.make)
}
export const simplePipeIn = UserRepository.Default.pipe(Layer.provide(Cache.Default))

export const cacheWithFs = Cache.Default.pipe(Layer.provide(FileSystem.Default))
// @filename: followSymbols.ts
import { cacheWithFs, DbConnection, simplePipeIn, UserRepository } from "./simple"
import { Layer } from "effect"

export const followSymbols = simplePipeIn.pipe(Layer.provide(DbConnection.Default))

export const moreComplex = UserRepository.Default.pipe(Layer.provide(cacheWithFs), Layer.merge(DbConnection.Default))
