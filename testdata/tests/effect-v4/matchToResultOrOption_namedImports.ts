// @effect-diagnostics *:off
// @effect-diagnostics matchToResultOrOption:warning
import type { Effect } from "effect"
import { Result } from "effect"
import { match } from "effect/Effect"

declare const fetchUser: (id: string) => Effect.Effect<{ readonly name: string }, string>

// MUST TRIGGER (no fix): no value binding in scope reaches Effect.result
export const namedMatch = match(fetchUser("u1"), {
  onFailure: Result.fail,
  onSuccess: Result.succeed
})
