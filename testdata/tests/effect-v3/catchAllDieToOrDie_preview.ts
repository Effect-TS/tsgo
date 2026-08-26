// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics catchAllDieToOrDie:warning
import * as Effect from "effect/Effect"

declare const readConfig: Effect.Effect<string, Error>

export const program = readConfig.pipe(
  Effect.catchAll((error) => Effect.die(error))
)
