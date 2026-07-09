// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics redundantOrDie:warning
import * as Effect from "effect/Effect"

declare const first: Effect.Effect<number, unknown>
declare const second: Effect.Effect<string, unknown>

export const program = Effect.gen(function*() {
  yield* first.pipe(Effect.orDie)

  return yield* second.pipe(Effect.orDie)
})
