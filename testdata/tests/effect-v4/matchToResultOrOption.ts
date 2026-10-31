// @effect-diagnostics *:off
// @effect-diagnostics matchToResultOrOption:warning
import { Data, Effect, Exit, Option, pipe, Result } from "effect"
import * as E from "effect/Effect"
import * as O from "effect/Option"
import { match as foldEffect } from "effect/Effect"
import { fail as failure, succeed as success } from "effect/Result"

class UserNotFound extends Data.TaggedError("UserNotFound")<{
  readonly id: string
}> {}

interface User {
  readonly name: string
}

declare const fetchUser: (id: string) => Effect.Effect<User, UserNotFound>
declare const withRequirements: Effect.Effect<User, UserNotFound, "Db">
declare const exitValue: Exit.Exit<User, UserNotFound>
declare const resultValue: Result.Result<User, UserNotFound>
declare const fallbackError: UserNotFound
declare const guestUser: User
declare const record: (value: unknown) => void

// ---------------------------------------------------------------------------
// Effect.match -> Effect.result
// ---------------------------------------------------------------------------

// MUST TRIGGER: data-first, identity arrows wrapping into Result (issue example)
export const dataFirstResultArrows = Effect.match(fetchUser("u1"), {
  onFailure: (e) => Result.fail(e),
  onSuccess: (user) => Result.succeed(user)
})

// MUST TRIGGER: data-first, constructor references passed point-free
export const dataFirstResultReferences = Effect.match(fetchUser("u1"), {
  onFailure: Result.fail,
  onSuccess: Result.succeed
})

// MUST TRIGGER: pipeable form
export const pipeableResult = fetchUser("u1").pipe(
  Effect.match({
    onFailure: Result.fail,
    onSuccess: Result.succeed
  })
)

// MUST TRIGGER: pipe function form
export const pipeFunctionResult = pipe(
  fetchUser("u1"),
  Effect.match({
    onFailure: (e) => Result.fail(e),
    onSuccess: (user) => Result.succeed(user)
  })
)

// MUST TRIGGER: handlers listed in reverse order (onSuccess first)
export const reversedPropertyOrder = Effect.match(fetchUser("u1"), {
  onSuccess: Result.succeed,
  onFailure: Result.fail
})

// MUST TRIGGER: mixed reference / identity arrow handlers
export const mixedHandlerForms = Effect.match(fetchUser("u1"), {
  onFailure: Result.fail,
  onSuccess: (user) => Result.succeed(user)
})

// MUST TRIGGER: annotated parameters and parenthesized bodies are still identity arrows
export const annotatedParameters = Effect.match(fetchUser("u1"), {
  onFailure: (e: UserNotFound) => (Result.fail(e)),
  onSuccess: (user: User) => (Result.succeed(user))
})

// MUST TRIGGER: block bodies whose only statement returns the reified value
export const singleReturnBlocks = Effect.match(fetchUser("u1"), {
  onFailure: (e) => {
    return Result.fail(e)
  },
  onSuccess: (user) => {
    return Result.succeed(user)
  }
})

// MUST TRIGGER: quoted property names
export const quotedProperties = Effect.match(fetchUser("u1"), {
  "onFailure": Result.fail,
  "onSuccess": Result.succeed
})

// MUST TRIGGER: requirements are threaded through by Effect.result as well
export const requirementsPreserved = withRequirements.pipe(
  Effect.match({
    onFailure: Result.fail,
    onSuccess: Result.succeed
  })
)

// MUST TRIGGER: Exit is an Effect, so Effect.match over an Exit is still Effect.result
export const overExit = Effect.match(exitValue, {
  onFailure: Result.fail,
  onSuccess: Result.succeed
})

// MUST TRIGGER: namespace import alias of the Effect module
export const namespaceAlias = E.match(fetchUser("u1"), {
  onFailure: Result.fail,
  onSuccess: Result.succeed
})

// MUST TRIGGER: renamed named imports for both Effect.match and the Result constructors
export const renamedImports = foldEffect(fetchUser("u1"), {
  onFailure: (e) => failure(e),
  onSuccess: success
})

// MUST TRIGGER: local constant alias of Effect.match
const matchAlias = Effect.match
export const constantAlias = fetchUser("u1").pipe(
  matchAlias({
    onFailure: Result.fail,
    onSuccess: Result.succeed
  })
)

// MUST TRIGGER: the match is one step in a longer flow
export const middleOfLongerFlow = fetchUser("u1").pipe(
  Effect.tap((user) => Effect.log(user.name)),
  Effect.match({
    onFailure: Result.fail,
    onSuccess: Result.succeed
  }),
  Effect.map((result) => Result.isSuccess(result))
)

// MUST TRIGGER: inside a generator
export const insideGen = Effect.gen(function*() {
  const result = yield* Effect.match(fetchUser("u1"), {
    onFailure: Result.fail,
    onSuccess: Result.succeed
  })
  return result
})

// ---------------------------------------------------------------------------
// Effect.match -> Effect.option
// ---------------------------------------------------------------------------

// MUST TRIGGER: data-first, discarding the error (issue example)
export const dataFirstOption = Effect.match(fetchUser("u1"), {
  onFailure: () => Option.none(),
  onSuccess: Option.some
})

// MUST TRIGGER: Option.none passed point-free as the failure handler
export const optionNoneReference = Effect.match(fetchUser("u1"), {
  onFailure: Option.none,
  onSuccess: Option.some
})

// MUST TRIGGER: pipeable form with an identity arrow for Option.some
export const pipeableOption = fetchUser("u1").pipe(
  Effect.match({
    onFailure: () => Option.none(),
    onSuccess: (user) => Option.some(user)
  })
)

// MUST TRIGGER: failure handler declares but ignores its parameter
export const ignoredFailureParameter = Effect.match(fetchUser("u1"), {
  onFailure: (_error) => Option.none(),
  onSuccess: Option.some
})

// MUST TRIGGER: explicit type argument on Option.none
export const explicitNoneTypeArgument = Effect.match(fetchUser("u1"), {
  onFailure: () => Option.none<User>(),
  onSuccess: Option.some
})

// MUST TRIGGER: Option module imported under an alias
export const optionNamespaceAlias = pipe(
  fetchUser("u1"),
  Effect.match({
    onFailure: () => O.none(),
    onSuccess: O.some
  })
)

// ---------------------------------------------------------------------------
// Effect.matchEffect with Effect.succeed-wrapped reifying constructors
// ---------------------------------------------------------------------------

// MUST TRIGGER: matchEffect, both branches Effect.succeed around Result constructors -> Effect.result
export const matchEffectResult = Effect.matchEffect(fetchUser("u1"), {
  onFailure: (e) => Effect.succeed(Result.fail(e)),
  onSuccess: (user) => Effect.succeed(Result.succeed(user))
})

// MUST TRIGGER: matchEffect, pipeable, Effect.succeed around Option constructors -> Effect.option
export const matchEffectOption = fetchUser("u1").pipe(
  Effect.matchEffect({
    onFailure: () => Effect.succeed(Option.none()),
    onSuccess: (user) => Effect.succeed(Option.some(user))
  })
)

// ---------------------------------------------------------------------------
// Effect.map(constructor) directly followed by Effect.catch(... Effect.succeed(constructor))
// ---------------------------------------------------------------------------

// MUST TRIGGER: pipeable map(Result.succeed) + catch(e => succeed(Result.fail(e))) -> Effect.result
export const mapCatchResult = fetchUser("u1").pipe(
  Effect.map(Result.succeed),
  Effect.catch((e) => Effect.succeed(Result.fail(e)))
)

// MUST TRIGGER: pipe function form, identity arrows, Option shape -> Effect.option
export const mapCatchOption = pipe(
  fetchUser("u1"),
  Effect.map((user) => Option.some(user)),
  Effect.catch(() => Effect.succeed(Option.none()))
)

// MUST TRIGGER: nested data-first form
export const mapCatchNestedDataFirst = Effect.catch(
  Effect.map(fetchUser("u1"), Result.succeed),
  (e) => Effect.succeed(Result.fail(e))
)

// ---------------------------------------------------------------------------
// Near misses: MUST NOT TRIGGER
// ---------------------------------------------------------------------------

// MUST NOT TRIGGER: Result.match has the same handler shape but folds a Result, not an Effect
export const resultMatch = Result.match(resultValue, {
  onFailure: () => Option.none(),
  onSuccess: Option.some
})

// MUST NOT TRIGGER: Exit.match is not Effect.match even though Exit is an Effect
export const exitMatch = Exit.match(exitValue, {
  onFailure: Result.fail,
  onSuccess: Result.succeed
})

// MUST NOT TRIGGER: user-defined object with a match method of the same shape
const Local = {
  match: <A, E, B, C>(self: Effect.Effect<A, E>, handlers: {
    readonly onFailure: (e: E) => B
    readonly onSuccess: (a: A) => C
  }) => Effect.match(self, handlers)
}
export const localMatch = Local.match(fetchUser("u1"), {
  onFailure: Result.fail,
  onSuccess: Result.succeed
})

// MUST NOT TRIGGER: the failure handler transforms the error (changes the error type)
export const transformsError = Effect.match(fetchUser("u1"), {
  onFailure: (e) => Result.fail(e.id),
  onSuccess: Result.succeed
})

// MUST NOT TRIGGER: the failure handler replaces the error with a constant
export const replacesError = Effect.match(fetchUser("u1"), {
  onFailure: () => Result.fail(fallbackError),
  onSuccess: Result.succeed
})

// MUST NOT TRIGGER: the success handler transforms the value
export const transformsValue = Effect.match(fetchUser("u1"), {
  onFailure: Result.fail,
  onSuccess: (user) => Result.succeed(user.name)
})

// MUST NOT TRIGGER: Option success handler transforms the value
export const transformsOptionValue = Effect.match(fetchUser("u1"), {
  onFailure: () => Option.none(),
  onSuccess: (user) => Option.some(user.name)
})

// MUST NOT TRIGGER: identity arrow returns a different binding than its parameter
export const wrongBinding = Effect.match(fetchUser("u1"), {
  onFailure: (_e) => Result.fail(fallbackError),
  onSuccess: (user) => Result.succeed(user)
})

// MUST NOT TRIGGER: destructured parameter rebuilds a new value
export const destructuredParameter = Effect.match(fetchUser("u1"), {
  onFailure: ({ id }) => Result.fail({ id }),
  onSuccess: Result.succeed
})

// MUST NOT TRIGGER: swapped constructors (that is a flip, not Effect.result)
export const swappedConstructors = Effect.match(fetchUser("u1"), {
  onFailure: Result.succeed,
  onSuccess: Result.fail
})

// MUST NOT TRIGGER: mixed containers (Option success with Result failure)
export const mixedContainers = Effect.match(fetchUser("u1"), {
  onFailure: Result.fail,
  onSuccess: Option.some
})

// MUST NOT TRIGGER: Option shape where the failure handler inspects the error
export const optionFailureUsesError = Effect.match(fetchUser("u1"), {
  onFailure: (e) => (e.id === "guest" ? Option.some(guestUser) : Option.none()),
  onSuccess: Option.some
})

// MUST NOT TRIGGER: Option shape with a side effect in the failure handler
export const optionFailureSideEffect = Effect.match(fetchUser("u1"), {
  onFailure: (e) => {
    record(e)
    return Option.none()
  },
  onSuccess: Option.some
})

// MUST NOT TRIGGER: Option.some on failure is not Effect.option
export const optionSomeOnFailure = Effect.match(fetchUser("u1"), {
  onFailure: (e) => Option.some(e),
  onSuccess: Option.some
})

// MUST NOT TRIGGER: handlers come from a variable, not an object literal
const handlers = {
  onFailure: Result.fail,
  onSuccess: Result.succeed
}
export const handlersFromVariable = Effect.match(fetchUser("u1"), handlers)

// MUST NOT TRIGGER: handlers object built with a spread
export const spreadHandlers = Effect.match(fetchUser("u1"), {
  ...handlers
})

// MUST NOT TRIGGER: same-named local Result shadowing the effect module
export const shadowedResult = (() => {
  const Result = {
    succeed: <A>(a: A) => ({ ok: true as const, a }),
    fail: <E>(e: E) => ({ ok: false as const, e })
  }
  return Effect.match(fetchUser("u1"), {
    onFailure: (e) => Result.fail(e),
    onSuccess: (user) => Result.succeed(user)
  })
})()

// MUST NOT TRIGGER: matchCause with Exit constructors (Effect.exit is out of scope for this rule)
export const matchCauseToExit = Effect.matchCause(fetchUser("u1"), {
  onFailure: Exit.failCause,
  onSuccess: Exit.succeed
})

// MUST NOT TRIGGER: matchEager is not rewritten to the non-eager Effect.result
export const matchEagerResult = Effect.matchEager(fetchUser("u1"), {
  onFailure: Result.fail,
  onSuccess: Result.succeed
})

// MUST NOT TRIGGER: matchEffect whose failure branch re-fails instead of reifying
export const matchEffectRefails = Effect.matchEffect(fetchUser("u1"), {
  onFailure: (e) => Effect.fail(e),
  onSuccess: (user) => Effect.succeed(Result.succeed(user))
})

// MUST NOT TRIGGER: matchEffect whose failure branch performs real work before reifying
export const matchEffectRealWork = Effect.matchEffect(fetchUser("u1"), {
  onFailure: (e) => Effect.as(Effect.log(e.id), Result.fail(e)),
  onSuccess: (user) => Effect.succeed(Result.succeed(user))
})

// MUST NOT TRIGGER: map(Result.succeed) and catch are not adjacent
export const mapCatchNotAdjacent = fetchUser("u1").pipe(
  Effect.map(Result.succeed),
  Effect.tap(() => Effect.log("mapped")),
  Effect.catch((e) => Effect.succeed(Result.fail(e)))
)

// MUST NOT TRIGGER: catch before map (the recovered value is mapped again)
export const catchThenMap = fetchUser("u1").pipe(
  Effect.catch((e) => Effect.succeed(e)),
  Effect.map(Result.succeed)
)

// MUST NOT TRIGGER: catchTag only recovers one error case
export const mapCatchTag = fetchUser("u1").pipe(
  Effect.map(Result.succeed),
  Effect.catchTag("UserNotFound", (e) => Effect.succeed(Result.fail(e)))
)

// MUST NOT TRIGGER: catchCause also reifies defects and interruptions
export const mapCatchCause = fetchUser("u1").pipe(
  Effect.map(Result.succeed),
  Effect.catchCause((cause) => Effect.succeed(Result.fail(cause)))
)

// MUST NOT TRIGGER: catch handler transforms the error
export const mapCatchTransformsError = fetchUser("u1").pipe(
  Effect.map(Result.succeed),
  Effect.catch((e) => Effect.succeed(Result.fail(e.id)))
)

// MUST NOT TRIGGER: map(Option.some) followed by a catch that re-fails
export const mapCatchRefails = fetchUser("u1").pipe(
  Effect.map(Option.some),
  Effect.catch((e) => Effect.fail(e))
)

// MUST NOT TRIGGER: map(Result.succeed) alone
export const mapOnly = fetchUser("u1").pipe(Effect.map(Result.succeed))

// MUST NOT TRIGGER: already using the dedicated combinators
export const alreadyResult = Effect.result(fetchUser("u1"))
export const alreadyOption = fetchUser("u1").pipe(Effect.option)
