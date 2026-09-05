// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics runOfExitToRunExit:warning
import { Effect } from "effect"

declare const program: Effect.Effect<number, Error>

export const exit = Effect.runPromise(Effect.exit(program))
