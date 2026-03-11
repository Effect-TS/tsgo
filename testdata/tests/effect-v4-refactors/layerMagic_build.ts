// refactor: 26:14-26:49, 31:14-31:36, 38:14-38:37, 45:14-45:32, 49:14-49:27, 51:14-51:36
import { Effect, Layer, ServiceMap } from "effect"

class DbConnection extends ServiceMap.Service<DbConnection>()("DbConnection", {
  make: Effect.succeed({})
}) {
  static Default = Layer.effect(this, this.make)
}
class FileSystem extends ServiceMap.Service<FileSystem>()("FileSystem", {
  make: Effect.succeed({})
}) {
  static Default = Layer.effect(this, this.make)
  static bothInAndOut = Layer.effect(FileSystem, FileSystem.asEffect())
}
class Cache extends ServiceMap.Service<Cache>()("Cache", {
  make: Effect.as(FileSystem.asEffect(), {})
}) {
  static Default = Layer.effect(this, this.make)
}
class UserRepository extends ServiceMap.Service<UserRepository>()("UserRepository", {
  make: Effect.as(Effect.andThen(DbConnection.asEffect(), Cache.asEffect()), {})
}) {
  static Default = Layer.effect(this, this.make)
}

export const expect_Cache_provideMergeFileSystem = [
  FileSystem.Default,
  Cache.Default
] as any as Layer.Layer<FileSystem>

export const prepareSomewhatComplex = [
  DbConnection.Default,
  Cache.Default,
  UserRepository.Default,
  FileSystem.Default
] as any as Layer.Layer<UserRepository | Cache>

export const prepareSomewhatComplex2 = [
  DbConnection.Default,
  Cache.Default,
  UserRepository.Default,
  FileSystem.Default
] as any as Layer.Layer<UserRepository | Cache | FileSystem>

export const provideRequireSame = [FileSystem.bothInAndOut, FileSystem.Default, Cache.Default] as any as Layer.Layer<
  Cache | FileSystem
>

export const tooLessOutput = [Cache.Default] as any as Layer.Layer<Cache>

export const missingImplementations = [UserRepository.Default, FileSystem.Default] as any as Layer.Layer<Cache>
