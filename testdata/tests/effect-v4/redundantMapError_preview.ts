// @effect-diagnostics *:off
// @effect-diagnostics redundantMapError:warning
import { Effect } from "effect"

declare const first: Effect.Effect<number, unknown>
declare const second: Effect.Effect<string, unknown>

class RepoError {
  constructor(readonly args: { cause: unknown }) {}
}

export const program = Effect.gen(function*() {
  yield* first.pipe(
    Effect.mapError((cause) => new RepoError({ cause }))
  )

  return yield* second.pipe(
    Effect.mapError((cause) => new RepoError({ cause }))
  )
})
