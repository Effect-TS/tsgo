import * as Fx from "effect/Effect"
import { match as matchNamed } from "effect/Effect"
import { Effect, Exit, Option, pipe } from "effect"
import { constFalse, constTrue } from "effect/Function"

declare const saveUser: Effect.Effect<void, Error>
declare const loadCount: Effect.Effect<number, string>

// MUST TRIGGER: data-first Effect.match, onSuccess true / onFailure false (issue example) -> Effect.isSuccess
export const saved = Effect.match(saveUser, {
  onFailure: () => false,
  onSuccess: () => true
})

// MUST TRIGGER: swapped literals (issue example) -> Effect.isFailure
export const failed = Effect.match(saveUser, {
  onFailure: () => true,
  onSuccess: () => false
})

// MUST TRIGGER: pipeable form inside .pipe -> Effect.isSuccess
export const pipeable = saveUser.pipe(Effect.match({
  onFailure: () => false,
  onSuccess: () => true
}))

// MUST TRIGGER: pipe function form -> Effect.isFailure
export const piped = pipe(
  loadCount,
  Effect.match({
    onFailure: () => true,
    onSuccess: () => false
  })
)

// MUST TRIGGER: point-free form used as a function value -> Effect.isSuccess
export const succeeded = Effect.match({
  onFailure: () => false,
  onSuccess: () => true
})

// MUST TRIGGER: data-last form applied directly -> Effect.isSuccess
export const dataLastApplied = Effect.match({
  onFailure: () => false,
  onSuccess: () => true
})(loadCount)

// MUST TRIGGER: Effect.matchEager, data-first -> Effect.isSuccess
export const eagerDataFirst = Effect.matchEager(saveUser, {
  onFailure: () => false,
  onSuccess: () => true
})

// MUST TRIGGER: Effect.matchEager, point-free (Effect's own isFailure definition shape) -> Effect.isFailure
export const eagerPointFree = Effect.matchEager({
  onFailure: () => true,
  onSuccess: () => false
})

// MUST TRIGGER: property order does not matter -> Effect.isSuccess
export const successFirst = Effect.match(loadCount, {
  onSuccess: () => true,
  onFailure: () => false
})

// MUST TRIGGER: parameters declared but never referenced -> Effect.isSuccess
export const ignoredParameters = Effect.match(loadCount, {
  onFailure: (_error) => false,
  onSuccess: (_value) => true
})

// MUST TRIGGER: one handler declares an unused parameter, the other none -> Effect.isFailure
export const mixedParameters = Effect.match(loadCount, {
  onFailure: (error) => true,
  onSuccess: () => false
})

// MUST TRIGGER: quoted property names -> Effect.isSuccess
export const quotedProperties = loadCount.pipe(Effect.match({
  "onFailure": () => false,
  "onSuccess": () => true
}))

// MUST TRIGGER: parenthesized boolean literal bodies -> Effect.isSuccess
export const parenthesizedBodies = Effect.match(loadCount, {
  onFailure: () => (false),
  onSuccess: () => (true)
})

// MUST TRIGGER: aliased namespace import -> Fx.isSuccess
export const aliasedModule = saveUser.pipe(Fx.match({
  onFailure: () => false,
  onSuccess: () => true
}))

// MUST TRIGGER: named import of match -> isSuccess
export const namedImport = matchNamed(saveUser, {
  onFailure: () => false,
  onSuccess: () => true
})

// MUST TRIGGER: the API reference is hidden behind a const alias -> Effect.isFailure
const matchAlias = Effect.match
export const aliasedMatch = saveUser.pipe(matchAlias({
  onFailure: () => true,
  onSuccess: () => false
}))

// MUST TRIGGER: inside Effect.gen with yield* -> Effect.isSuccess
export const insideGen = Effect.gen(function*() {
  const ok = yield* Effect.match(saveUser, {
    onFailure: () => false,
    onSuccess: () => true
  })
  return ok
})

// MUST TRIGGER: in the middle of a longer pipe chain -> Effect.isSuccess
export const chained = loadCount.pipe(
  Effect.tap(() => Effect.void),
  Effect.match({
    onFailure: () => false,
    onSuccess: () => true
  }),
  Effect.map((ok) => (ok ? "ok" : "ko"))
)

// MUST NOT TRIGGER: already using the dedicated APIs
export const alreadyIsSuccess = Effect.isSuccess(saveUser)
export const alreadyIsFailure = saveUser.pipe(Effect.isFailure)

// MUST NOT TRIGGER: Effect.matchCause converts defects too; isSuccess would change defect semantics
export const matchCause = Effect.matchCause(saveUser, {
  onFailure: () => false,
  onSuccess: () => true
})

// MUST NOT TRIGGER: Effect.matchCauseEager, same reason (pipeable form)
export const matchCauseEager = saveUser.pipe(Effect.matchCauseEager({
  onFailure: () => true,
  onSuccess: () => false
}))

// MUST NOT TRIGGER: Effect.matchEffect is a different API (effectful handlers)
export const matchEffect = Effect.matchEffect(saveUser, {
  onFailure: () => Effect.succeed(false),
  onSuccess: () => Effect.succeed(true)
})

// MUST NOT TRIGGER: both handlers return the same literal (not complementary)
export const bothTrue = Effect.match(saveUser, {
  onFailure: () => true,
  onSuccess: () => true
})
export const bothFalse = Effect.match(saveUser, {
  onFailure: () => false,
  onSuccess: () => false
})

// MUST NOT TRIGGER: handlers actually use their parameter
export const usesValue = Effect.match(loadCount, {
  onFailure: () => false,
  onSuccess: (value) => value > 0
})
export const usesError = Effect.match(loadCount, {
  onFailure: (error) => error.length === 0,
  onSuccess: () => true
})

// MUST NOT TRIGGER: negations and other non-literal boolean expressions
export const negations = Effect.match(saveUser, {
  onFailure: () => !true,
  onSuccess: () => !false
})

// MUST NOT TRIGGER: boolean identifiers / constants are not bare literals
const yes = true
const no = false
export const identifiers = Effect.match(saveUser, {
  onFailure: () => no,
  onSuccess: () => yes
})

// MUST NOT TRIGGER: non-boolean literals
export const numericLiterals = Effect.match(saveUser, {
  onFailure: () => 0,
  onSuccess: () => 1
})
export const stringLiterals = Effect.match(saveUser, {
  onFailure: () => "false",
  onSuccess: () => "true"
})

// MUST NOT TRIGGER: type assertions around the literal
export const asserted = Effect.match(saveUser, {
  onFailure: () => false as boolean,
  onSuccess: () => true as const
})

// MUST NOT TRIGGER: block bodies are outside the bare-literal shape
export const blockBodies = Effect.match(saveUser, {
  onFailure: () => {
    return false
  },
  onSuccess: () => {
    return true
  }
})

// MUST NOT TRIGGER: function expressions and methods are not arrow functions
export const functionExpressions = Effect.match(saveUser, {
  onFailure: function() {
    return false
  },
  onSuccess: function() {
    return true
  }
})
export const methods = Effect.match(saveUser, {
  onFailure() {
    return false
  },
  onSuccess() {
    return true
  }
})

// MUST NOT TRIGGER: point-free constFalse / constTrue handlers are not inline arrow functions
export const constHandlers = Effect.match(saveUser, {
  onFailure: constFalse,
  onSuccess: constTrue
})

// MUST NOT TRIGGER: destructured / defaulted parameters are not plain ignored parameters
export const destructuredParameter = Effect.match(Effect.fail({ code: 1 }), {
  onFailure: ({ code: _code }) => false,
  onSuccess: () => true
})
export const defaultParameter = Effect.match(loadCount, {
  onFailure: (_error = "none") => false,
  onSuccess: () => true
})

// MUST NOT TRIGGER: options are not an inline object literal
const handlers = {
  onFailure: () => false,
  onSuccess: () => true
}
export const optionsVariable = Effect.match(saveUser, handlers)
export const optionsSpread = Effect.match(saveUser, { ...handlers })

// MUST NOT TRIGGER: Option.match and Exit.match are different modules
export const optionMatch = Option.some(1).pipe(Option.match({
  onNone: () => false,
  onSome: () => true
}))
declare const someExit: Exit.Exit<number, string>
export const exitMatch = Exit.match(someExit, {
  onFailure: () => false,
  onSuccess: () => true
})

// MUST NOT TRIGGER: unrelated match API with the same shape
const unrelated = {
  match: <A, B>(_self: unknown, options: { onFailure: () => A; onSuccess: () => B }): A | B => options.onSuccess()
}
export const unrelatedMatch = unrelated.match(saveUser, {
  onFailure: () => false,
  onSuccess: () => true
})

// MUST NOT TRIGGER: Effect.exit + Effect.map(Exit.isSuccess / Exit.isFailure) is NOT equivalent
// to Effect.isSuccess / Effect.isFailure. Effect.exit captures every cause, so a defect
// (Effect.die) or an interruption yields false from Exit.isSuccess (true from Exit.isFailure),
// while the match-based Effect.isSuccess / Effect.isFailure let defects and interruptions
// propagate. Verified at runtime against the bundled Effect (v3 and v4): same types, different
// behaviour, for the same reason Effect.matchCause is excluded.

// MUST NOT TRIGGER: data-first map over Effect.exit with Exit.isSuccess (defect/interrupt -> false)
export const exitDataFirst = Effect.map(Effect.exit(saveUser), Exit.isSuccess)

// MUST NOT TRIGGER: data-first map over Effect.exit with Exit.isFailure (defect/interrupt -> true)
export const exitDataFirstFailure = Effect.map(Effect.exit(saveUser), Exit.isFailure)

// MUST NOT TRIGGER: Effect.exit(...) receiver piped into Effect.map(Exit.isSuccess)
export const exitReceiverPipeable = Effect.exit(saveUser).pipe(Effect.map(Exit.isSuccess))

// MUST NOT TRIGGER: adjacent Effect.exit / Effect.map(Exit.isSuccess) pipeable steps
export const exitPipeableSteps = saveUser.pipe(Effect.exit, Effect.map(Exit.isSuccess))

// MUST NOT TRIGGER: adjacent Effect.exit / Effect.map(Exit.isFailure) steps in the pipe function
export const exitPipeSteps = pipe(loadCount, Effect.exit, Effect.map(Exit.isFailure))

// MUST NOT TRIGGER: Effect.exit mapped through a lambda calling Exit.isSuccess (same defect difference)
export const exitLambda = Effect.map(Effect.exit(saveUser), (exit) => Exit.isSuccess(exit))

// MUST NOT TRIGGER: other Exit predicate mapped over Effect.exit
export const exitOtherPredicate = Effect.map(Effect.exit(saveUser), Exit.hasDies)

// MUST NOT TRIGGER: Effect.exit and Effect.map(Exit.isSuccess) separated by another step
export const exitNotAdjacent = saveUser.pipe(
  Effect.exit,
  Effect.tap(() => Effect.void),
  Effect.map(Exit.isSuccess)
)

// MUST NOT TRIGGER: Exit.isSuccess mapped over an effect that already yields an Exit
declare const exitEffect: Effect.Effect<Exit.Exit<number, string>>
export const exitFromVariable = Effect.map(exitEffect, Exit.isSuccess)
