// @effect-diagnostics *:off
// @effect-diagnostics onExitFailureToOnError:suggestion
import { Cause, Clock, Effect, Exit, pipe } from "effect"

declare const program: Effect.Effect<number, string>
declare const otherExit: Exit.Exit<number, string>
declare const enabled: boolean
declare const started: number
declare const handler: (exit: Exit.Exit<number, string>) => Effect.Effect<void>
declare const isFailed: (exit: Exit.Exit<number, string>) => boolean
let counter = 0

// ---------------------------------------------------------------------------
// MUST TRIGGER
// ---------------------------------------------------------------------------

// MUST TRIGGER: the canonical shape, pipeable, conditional expression.
export const pipeableTernary = program.pipe(
  Effect.onExit((exit) =>
    Exit.isFailure(exit)
      ? Effect.logError("failed", { cause: exit.cause })
      : Effect.void
  )
)

// MUST TRIGGER: pipe(...) spelling.
export const pipeTernary = pipe(
  program,
  Effect.onExit((exit) => Exit.isFailure(exit) ? Effect.logError("failed", { cause: exit.cause }) : Effect.void)
)

// MUST TRIGGER: data-first spelling.
export const dataFirst = Effect.onExit(
  program,
  (exit) => Exit.isFailure(exit) ? Effect.logError("failed", { cause: exit.cause }) : Effect.void
)

// MUST TRIGGER: success check that does nothing, fallback acts.
export const successNoop = program.pipe(
  Effect.onExit((exit) => Exit.isSuccess(exit) ? Effect.void : Effect.logError("failed", { cause: exit.cause }))
)

// MUST TRIGGER: negated success check, `if (...) return X; return Effect.void` (issue example).
export const negatedSuccessIfReturn = program.pipe(
  Effect.onExit((exit) => {
    if (!Exit.isSuccess(exit)) return Effect.logError("failed", { cause: exit.cause })
    return Effect.void
  })
)

// MUST TRIGGER: negated failure check that does nothing, fallback acts.
export const negatedFailureNoop = program.pipe(
  Effect.onExit((exit) => !Exit.isFailure(exit) ? Effect.void : Effect.logError("failed", { cause: exit.cause }))
)

// MUST TRIGGER: `=== true`, X does not use the cause (issue example).
export const strictTrueUnusedCause = program.pipe(
  Effect.onExit((exit) =>
    Exit.isFailure(exit) === true
      ? Effect.gen(function*() {
        const now = yield* Clock.currentTimeMillis
        yield* Effect.logError("readiness failed", { elapsedMs: now - started })
      })
      : Effect.void
  )
)

// MUST TRIGGER: `isSuccess(...) === false` acts on failure.
export const successStrictFalse = program.pipe(
  Effect.onExit((exit) => Exit.isSuccess(exit) === false ? Effect.logError("failed", { cause: exit.cause }) : Effect.void)
)

// MUST TRIGGER: `isFailure(...) === false` does nothing, fallback acts.
export const failureStrictFalseNoop = program.pipe(
  Effect.onExit((exit) => Exit.isFailure(exit) === false ? Effect.void : Effect.logError("failed", { cause: exit.cause }))
)

// MUST TRIGGER: parenthesized condition.
export const parenthesized = program.pipe(
  Effect.onExit((exit) => (Exit.isFailure(exit)) ? Effect.logError("failed", { cause: exit.cause }) : Effect.void)
)

// MUST TRIGGER: negation of a parenthesized strict comparison.
export const negatedParenthesizedStrict = program.pipe(
  Effect.onExit((exit) => !(Exit.isSuccess(exit) === true) ? Effect.logError("failed", { cause: exit.cause }) : Effect.void)
)

// MUST TRIGGER: the check written as pipe(exit, Exit.isFailure).
// (this spelling does not narrow `exit`, so X here does not read the cause)
export const checkViaPipe = program.pipe(
  Effect.onExit((exit) => pipe(exit, Exit.isFailure) ? Effect.logError("failed") : Effect.void)
)

// MUST TRIGGER: the check written as exit.pipe(Exit.isSuccess).
export const checkViaPipeable = program.pipe(
  Effect.onExit((exit) => exit.pipe(Exit.isSuccess) ? Effect.void : Effect.logError("failed"))
)

// MUST TRIGGER: tag comparison against "Failure".
export const tagFailure = program.pipe(
  Effect.onExit((exit) => exit._tag === "Failure" ? Effect.logError("failed", { cause: exit.cause }) : Effect.void)
)

// MUST TRIGGER: tag comparison against "Success" that does nothing.
export const tagSuccessNoop = program.pipe(
  Effect.onExit((exit) => exit._tag === "Success" ? Effect.void : Effect.logError("failed", { cause: exit.cause }))
)

// MUST TRIGGER: if / else with returning branches.
export const ifElse = program.pipe(
  Effect.onExit((exit) => {
    if (Exit.isFailure(exit)) {
      return Effect.logError("failed", { cause: exit.cause })
    } else {
      return Effect.void
    }
  })
)

// MUST TRIGGER: a block containing only a returned conditional.
export const returnedTernary = program.pipe(
  Effect.onExit((exit) => {
    return Exit.isFailure(exit) ? Effect.logError("failed", { cause: exit.cause }) : Effect.void
  })
)

// MUST TRIGGER: switch on the tag with one "Failure" case and a no-op default.
export const switchFailure = program.pipe(
  Effect.onExit((exit) => {
    switch (exit._tag) {
      case "Failure":
        return Effect.logError("failed", { cause: exit.cause })
      default:
        return Effect.void
    }
  })
)

// MUST TRIGGER: switch on the tag with a no-op "Success" case and an acting default.
export const switchSuccessNoop = program.pipe(
  Effect.onExit((exit) => {
    switch (exit._tag) {
      case "Success":
        return Effect.void
      default:
        return Effect.logError("failed", { cause: exit.cause })
    }
  })
)

// MUST TRIGGER: the default clause comes first; every clause returns, so its position does not change which one runs.
export const defaultFirstSwitch = program.pipe(
  Effect.onExit((exit) => {
    switch (exit._tag) {
      default:
        return Effect.void
      case "Failure":
        return Effect.logError("failed", { cause: exit.cause })
    }
  })
)

// MUST TRIGGER: an inline generic callback whose type parameter X does not refer to.
export const genericCallback = program.pipe(
  Effect.onExit(<A>(exit: Exit.Exit<A, string>) => Exit.isFailure(exit) ? Effect.logError(exit.cause) : Effect.void)
)

// MUST TRIGGER: a generic function expression callback.
export const genericFunctionExpression = program.pipe(
  Effect.onExit(function<A>(exit: Exit.Exit<A, string>) {
    if (Exit.isFailure(exit)) return Effect.logError("failed", { cause: exit.cause })
    return Effect.void
  })
)

// MUST TRIGGER: function expression callback.
export const functionExpression = program.pipe(
  Effect.onExit(function(exit) {
    if (Exit.isFailure(exit)) return Effect.logError("failed", { cause: exit.cause })
    return Effect.void
  })
)

// MUST TRIGGER: the parameter name does not matter, the check must target it.
export const otherParameterName = program.pipe(
  Effect.onExit((result) => Exit.isFailure(result) ? Effect.logError("failed", { cause: result.cause }) : Effect.void)
)

// MUST TRIGGER: Effect.onExit as an Effect.fn pipeline argument (an `effectFn` step).
export const insideEffectFn = Effect.fn("insideEffectFn")(
  function*() {
    return yield* program
  },
  Effect.onExit((exit) => Exit.isFailure(exit) ? Effect.logError("failed", { cause: exit.cause }) : Effect.void)
)

// MUST TRIGGER: the whole exit is logged (issue example).
export const exitUsedDirectly = program.pipe(
  Effect.onExit((exit) => Exit.isFailure(exit) ? Effect.logError("failed", { exit }) : Effect.void)
)

// ---------------------------------------------------------------------------
// MUST NOT TRIGGER
// ---------------------------------------------------------------------------

// MUST NOT TRIGGER: already Effect.onError.
export const alreadyOnError = program.pipe(
  Effect.onError((cause) => Effect.logError("failed", { cause }))
)

// MUST NOT TRIGGER: tapCause is a different operator (and not equivalent).
export const tapCause = program.pipe(
  Effect.tapCause((cause) => Effect.logError("failed", { cause }))
)

// MUST NOT TRIGGER: the success branch does something (issue example).
export const successActs = program.pipe(
  Effect.onExit((exit) => Exit.isFailure(exit) ? Effect.logError("failed") : Effect.log("done"))
)

// MUST NOT TRIGGER: only some failures are handled (issue example).
export const partialFailure = program.pipe(
  Effect.onExit((exit) =>
    Exit.isFailure(exit) && Cause.hasInterruptsOnly(exit.cause)
      ? Effect.log("interrupted")
      : Effect.void
  )
)

// MUST NOT TRIGGER: acts only on success (polarity near-miss).
export const actsOnSuccess = program.pipe(
  Effect.onExit((exit) => Exit.isSuccess(exit) ? Effect.log("done") : Effect.void)
)

// MUST NOT TRIGGER: the failure check does nothing and the fallback acts only on success (polarity near-miss).
export const failureNoopActsOnSuccess = program.pipe(
  Effect.onExit((exit) => Exit.isFailure(exit) ? Effect.void : Effect.log("done"))
)

// MUST NOT TRIGGER: negated failure acts only on success (polarity near-miss).
export const negatedFailureActs = program.pipe(
  Effect.onExit((exit) => !Exit.isFailure(exit) ? Effect.log("done") : Effect.void)
)

// MUST NOT TRIGGER: `isFailure(...) === false` acts only on success (polarity near-miss).
export const failureStrictFalseActs = program.pipe(
  Effect.onExit((exit) => Exit.isFailure(exit) === false ? Effect.log("done") : Effect.void)
)

// MUST NOT TRIGGER: tag "Success" acts only on success (polarity near-miss).
export const tagSuccessActs = program.pipe(
  Effect.onExit((exit) => exit._tag === "Success" ? Effect.log("done") : Effect.void)
)

// MUST NOT TRIGGER: switch where the "Success" case acts.
export const switchSuccessActs = program.pipe(
  Effect.onExit((exit) => {
    switch (exit._tag) {
      case "Success":
        return Effect.log("done")
      default:
        return Effect.void
    }
  })
)

// MUST NOT TRIGGER: both branches act.
export const bothAct = program.pipe(
  Effect.onExit((exit) =>
    Exit.isFailure(exit) ? Effect.logError("failed", { cause: exit.cause }) : Effect.log("done")
  )
)

// MUST NOT TRIGGER: the fallback is not literally Effect.void.
export const fallbackNotVoid = program.pipe(
  Effect.onExit((exit) =>
    Exit.isFailure(exit) ? Effect.logError("failed", { cause: exit.cause }) : Effect.succeed(undefined)
  )
)

// MUST NOT TRIGGER: the fallback is Effect.asVoid of something, which still runs it.
export const fallbackAsVoid = program.pipe(
  Effect.onExit((exit) =>
    Exit.isFailure(exit) ? Effect.logError("failed", { cause: exit.cause }) : Effect.asVoid(Effect.log("done"))
  )
)

// MUST NOT TRIGGER: the check targets another exit, not the callback parameter.
export const otherExitChecked = program.pipe(
  Effect.onExit((_exit) => Exit.isFailure(otherExit) ? Effect.logError("failed") : Effect.void)
)

// MUST NOT TRIGGER: the condition is unrelated to the exit.
export const unrelatedCondition = program.pipe(
  Effect.onExit((_exit) => enabled ? Effect.logError("failed") : Effect.void)
)

// MUST NOT TRIGGER: a user-defined predicate, not Exit.isFailure.
export const customPredicate = program.pipe(
  Effect.onExit((exit) => isFailed(exit) ? Effect.logError("failed") : Effect.void)
)

// MUST NOT TRIGGER: three-way dispatch.
export const multiBranch = program.pipe(
  Effect.onExit((exit) =>
    Exit.isFailure(exit)
      ? Effect.logError("failed")
      : enabled
      ? Effect.log("done")
      : Effect.void
  )
)

// MUST NOT TRIGGER: switch with two explicit cases instead of one case and a default.
export const switchTwoCases = program.pipe(
  Effect.onExit((exit) => {
    switch (exit._tag) {
      case "Failure":
        return Effect.logError("failed", { cause: exit.cause })
      case "Success":
        return Effect.void
    }
  })
)

// MUST NOT TRIGGER: a statement with side effects runs before the dispatch, on every exit.
export const sideEffectBefore = program.pipe(
  Effect.onExit((exit) => {
    counter++
    if (Exit.isFailure(exit)) return Effect.logError("failed", { cause: exit.cause })
    return Effect.void
  })
)

// MUST NOT TRIGGER: the parameter is reassigned before the check.
export const reassigned = program.pipe(
  Effect.onExit((exit) => {
    exit = otherExit
    if (Exit.isFailure(exit)) return Effect.logError("failed", { cause: exit.cause })
    return Effect.void
  })
)

// MUST NOT TRIGGER: a defaulted parameter is not the untouched exit.
export const defaulted = program.pipe(
  Effect.onExit((exit = otherExit) => Exit.isFailure(exit) ? Effect.logError("failed") : Effect.void)
)

// MUST NOT TRIGGER: the callback is not an inline function.
export const namedHandler = program.pipe(Effect.onExit(handler))

// MUST NOT TRIGGER: the callback ignores the exit entirely.
export const ignoresExit = program.pipe(Effect.onExit(() => Effect.log("done")))
