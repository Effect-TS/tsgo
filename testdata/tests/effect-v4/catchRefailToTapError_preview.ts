// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics catchRefailToTapError:warning
import { Effect } from "effect"

declare const save: Effect.Effect<void, Error>
declare const rollback: Effect.Effect<void>

export const program = save.pipe(
  Effect.catch(error => rollback.pipe(Effect.andThen(Effect.fail(error))))
)
