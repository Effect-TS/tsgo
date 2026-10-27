// @effect-diagnostics *:off
// @effect-diagnostics flatMapIdentityToFlatten:warning
import { Effect } from "effect"

declare const loadTask: Effect.Effect<Effect.Effect<string>>

export const program = loadTask.pipe(Effect.flatMap((inner) => inner))
