// @effect-diagnostics *:off
// @effect-diagnostics matchToResultOrOption:warning
import { Data, Effect, Option, pipe, Result } from "effect"

class UserNotFound extends Data.TaggedError("UserNotFound")<{
  readonly id: string
}> {}

interface User {
  readonly name: string
}

declare const fetchUser: (id: string) => Effect.Effect<User, UserNotFound>
declare const fallbackError: UserNotFound

// MUST TRIGGER: curried application of the pipeable form
export const curried = Effect.match({
  onFailure: Result.fail,
  onSuccess: Result.succeed
})(fetchUser("u1"))

// MUST TRIGGER: trailing step of Effect.fn
export const effectFnStep = Effect.fn("effectFnStep")(
  function*(id: string) {
    return yield* fetchUser(id)
  },
  Effect.match({
    onFailure: Result.fail,
    onSuccess: Result.succeed
  })
)

// MUST TRIGGER: trailing step of Effect.fnUntraced
export const effectFnUntracedStep = Effect.fnUntraced(
  function*(id: string) {
    return yield* fetchUser(id)
  },
  Effect.match({
    onFailure: () => Option.none(),
    onSuccess: Option.some
  })
)

// MUST TRIGGER: map + catch pair split across two pipe calls
export const splitPipes = fetchUser("u1")
  .pipe(Effect.map(Result.succeed))
  .pipe(Effect.catch((e) => Effect.succeed(Result.fail(e))))

// MUST TRIGGER: map + catch pair as trailing steps of Effect.fn
export const effectFnPair = Effect.fn("effectFnPair")(
  function*(id: string) {
    return yield* fetchUser(id)
  },
  Effect.map(Option.some),
  Effect.catch(() => Effect.succeed(Option.none()))
)

// MUST TRIGGER: function expression handlers
export const functionExpressions = Effect.match(fetchUser("u1"), {
  onFailure: function(e) {
    return Result.fail(e)
  },
  onSuccess: function(user) {
    return Result.succeed(user)
  }
})

// MUST TRIGGER (no fix): explicit type arguments on pipe depend on the old output type
export const typedPipe = pipe<
  Effect.Effect<User, UserNotFound>,
  Effect.Effect<Result.Result<never, UserNotFound> | Result.Result<User, never>>
>(
  fetchUser("u1"),
  Effect.match({
    onFailure: Result.fail,
    onSuccess: Result.succeed
  })
)

// MUST NOT TRIGGER: async handlers return promises, not the reified value
export const asyncHandlers = Effect.match(fetchUser("u1"), {
  onFailure: async (e) => Result.fail(e),
  onSuccess: async (user) => Result.succeed(user)
})

// MUST NOT TRIGGER: explicit type arguments on the forwarded constructor
export const constructorTypeArguments = Effect.match(fetchUser("u1"), {
  onFailure: (e) => Result.fail<UserNotFound>(e),
  onSuccess: Result.succeed
})

// MUST NOT TRIGGER: explicit type arguments on Effect.succeed in a matchEffect branch
export const succeedTypeArguments = Effect.matchEffect(fetchUser("u1"), {
  onFailure: (e) => Effect.succeed<Result.Result<User, UserNotFound>>(Result.fail(e)),
  onSuccess: (user) => Effect.succeed(Result.succeed(user))
})

// MUST NOT TRIGGER: failure handler with a default parameter
export const defaultParameter = Effect.match(fetchUser("u1"), {
  onFailure: (_e = fallbackError) => Option.none(),
  onSuccess: Option.some
})

// MUST NOT TRIGGER: a rest parameter collects the error into an array, so Result.fail(e) wraps [error]
export const restParameter = Effect.matchEffect(fetchUser("u1"), {
  onFailure: (...e) => Effect.succeed(Result.fail(e)),
  onSuccess: (user) => Effect.succeed(Result.succeed(user))
})

// MUST NOT TRIGGER: an unapplied pipeable match is not applied to an Effect yet
export const unappliedFold = Effect.match({
  onFailure: Result.fail,
  onSuccess: Result.succeed
})
