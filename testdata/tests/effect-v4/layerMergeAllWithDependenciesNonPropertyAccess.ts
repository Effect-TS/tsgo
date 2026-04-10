import { Effect, Layer, Context } from "effect"

export class DbConnection extends Context.Service<DbConnection>()("DbConnection", {
  make: Effect.succeed({})
}) {
  static Default = Layer.effect(this, this.make)
}
export class FileSystem extends Context.Service<FileSystem>()("FileSystem", {
  make: Effect.succeed({})
}) {
  static Default = Layer.effect(this, this.make)
}
export class Cache extends Context.Service<Cache>()("Cache", {
  make: Effect.as(FileSystem.asEffect(), {})
}) {
  static Default = Layer.effect(this, this.make)
}

// Call mergeAll via destructuring — not a PropertyAccessExpression.
// The analyzer should skip this gracefully (no diagnostic, no panic).
const { mergeAll } = Layer
export const shouldNotWarnDestructured = mergeAll(
  DbConnection.Default,
  FileSystem.Default,
  Cache.Default
)
