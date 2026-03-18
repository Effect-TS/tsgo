import { Effect, Layer, ServiceMap, Data } from "effect"

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

export const liveWithPipeable = UserRepository.Default.pipe(
  Layer.provide(Cache.Default),
  Layer.provide(FileSystem.Default),
  Layer.merge(DbConnection.Default)
)

const program = Effect.gen(function*(){
    const fs = yield* FileSystem
    yield* Effect.addFinalizer(() => Effect.log("Finalizing file system"))
    return fs
})

program.pipe(
    Effect.provide(liveWithPipeable),
    Effect.scoped,
    Effect.runPromise
)