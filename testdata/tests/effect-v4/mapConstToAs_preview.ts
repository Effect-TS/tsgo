// @effect-diagnostics *:off
// @effect-diagnostics mapConstToAs:warning
import { Effect } from "effect"

const saveUser = (name: string): Effect.Effect<number> => Effect.succeed(name.length)

export const saved = saveUser("alice").pipe(Effect.map(() => "saved"))
