// @effect-diagnostics *:off
// @effect-diagnostics redundantOrDie:warning
import { Effect } from "effect"

declare const first: Effect.Effect<number, unknown>
declare const second: Effect.Effect<string, unknown>

export const program = Effect.gen(function*() {
  yield* first.pipe(Effect.orDie)

  return yield* second.pipe(Effect.orDie)
})
