// @effect-diagnostics *:off
// @effect-diagnostics matchToResultOrOption:warning
import { Data, Effect, Result } from "effect"

class UserNotFound extends Data.TaggedError("UserNotFound")<{
  readonly id: string
}> {}

declare const fetchUser: (id: string) => Effect.Effect<{ readonly name: string }, UserNotFound>

// MUST TRIGGER: both handlers only reify the outcome, which is Effect.result
export const userResult = Effect.match(fetchUser("u1"), {
  onFailure: (e) => Result.fail(e),
  onSuccess: (user) => Result.succeed(user)
})
