// @Filename: /test.ts
import { Effect, Layer, ServiceMap, Data, Schema } from "effect"

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

export const expect = UserRepository.Default

export const simplePipeIn = UserRepository.Default.pipe(Layer.provide(Cache.Default))

export const liveWithPipeable = UserRepository.Default.pipe(
  Layer.provideMerge(Cache.Default),
  Layer.merge(DbConnection.Default)
)

export const cacheWithFs = Cache.Default.pipe(Layer.provide(FileSystem.Default))

class NotFound extends Data.TaggedError("NotFound")<{
  readonly resource: string
}> {}

class Forbidden extends Data.TaggedError("Forbidden")<{
  readonly reason: string
}> {}

export const attempt = Effect.try({
  try: () => JSON.parse("not a valid JSON string"),
  catch: (error) => new NotFound({ resource: "user" }) /* <- this should not appear in the errors list */
})

const myArray: ServiceMap.Key<any, any>[] = []
for(const x of myArray) { // <- x variable inside for of should not appear in services list
  console.log(x)
}

// should not appear in errors list
export type ErrorSchema<A> = A extends { readonly ["TypeId"]: { readonly error: infer E } }
  ? E extends Schema.Top ? E : never
  : never

export class MySchemaClass extends Schema.Class<MySchemaClass>("MySchemaClass")({
  name: Schema.String,
  age: Schema.Number
}) {}

export const MyUser = Schema.Struct({
  name: Schema.String,
  age: Schema.Number
})