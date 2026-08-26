// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics catchDieToOrDie:warning
import { Effect } from "effect"

declare const readConfig: Effect.Effect<string, Error>

export const program = readConfig.pipe(
  Effect.catch((error) => Effect.die(error))
)
