// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics provideMergeAllToArray:warning
import { Context, Effect, Layer } from "effect"

class Db extends Context.Tag("Db")<Db, { readonly query: (sql: string) => Effect.Effect<string> }>() {}
class Logger extends Context.Tag("Logger")<Logger, { readonly log: (line: string) => Effect.Effect<void> }>() {}

declare const DbLayer: Layer.Layer<Db>
declare const LoggerLayer: Layer.Layer<Logger>
declare const program: Effect.Effect<void, never, Db | Logger>

// MUST TRIGGER: Effect.provide already merges an array of layers
export const main = program.pipe(Effect.provide(Layer.mergeAll(DbLayer, LoggerLayer)))
