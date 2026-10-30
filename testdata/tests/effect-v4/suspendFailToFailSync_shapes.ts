// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics suspendFailToFailSync:warning
import { Effect, pipe } from "effect"
import { fail, suspend } from "effect/Effect"
import type { Pipeable } from "effect/Pipeable"

interface PipeableError extends Pipeable {
  readonly _tag: "PipeableError"
}

declare const error: PipeableError
declare const defect: unknown
declare const code: number
declare const cond: boolean

// MUST TRIGGER: suspend as a call, fail as a call -> Effect.failSync(() => error)
export const callFailCall = Effect.suspend(() => Effect.fail(error))

// MUST TRIGGER: suspend as a call, fail as a Function.pipe step -> Effect.failSync(() => error)
export const callFailPipe = Effect.suspend(() => pipe(error, Effect.fail))

// MUST TRIGGER: suspend as a call, fail as a .pipe step -> Effect.failSync(() => error)
export const callFailMethodPipe = Effect.suspend(() => error.pipe(Effect.fail))

// MUST TRIGGER: suspend as a Function.pipe step, fail as a call -> pipe(() => error, Effect.failSync)
export const pipeFailCall = pipe(() => Effect.fail(error), Effect.suspend)

// MUST TRIGGER: suspend and fail as Function.pipe steps -> pipe(() => error, Effect.failSync)
export const pipeFailPipe = pipe(() => pipe(error, Effect.fail), Effect.suspend)

// MUST TRIGGER: suspend as a Function.pipe step, fail as a .pipe step -> pipe(() => error, Effect.failSync)
export const pipeFailMethodPipe = pipe(() => error.pipe(Effect.fail), Effect.suspend)

// MUST TRIGGER: the steps after the suspend are kept
// -> pipe(() => error, Effect.failSync, Effect.catch(() => Effect.succeed(0)))
export const pipeSuspendThenCatch = pipe(
  () => Effect.fail(error),
  Effect.suspend,
  Effect.catch(() => Effect.succeed(0))
)

// MUST TRIGGER: named imports in a pipe fall back to the file's Effect binding
// -> pipe(() => error, Effect.failSync)
export const pipeNamedImports = pipe(() => fail(error), suspend)

// MUST TRIGGER: an explicit type argument on the fail is carried over to the pipe step
// -> pipe(() => error, Effect.failSync<PipeableError | string>)
export const pipeInnerTypeArgument = pipe(() => Effect.fail<PipeableError | string>(error), Effect.suspend)

// MUST TRIGGER: the error value is computed by a call -> Effect.failSync(() => String(code))
export const computedValue = Effect.suspend(() => Effect.fail(String(code)))

// MUST TRIGGER: the error value is computed by an earlier pipe step, which stays in the thunk
// -> Effect.failSync(() => pipe(code, String))
export const computedPipeValue = Effect.suspend(() => pipe(code, String, Effect.fail))

// MUST TRIGGER: block body with a piped fail -> Effect.failSync(() => { return error })
export const blockFailPipe = Effect.suspend(() => {
  return pipe(error, Effect.fail)
})

// MUST TRIGGER: parenthesized piped fail -> Effect.failSync(() => error)
export const parenthesizedFailPipe = Effect.suspend(() => (pipe(error, Effect.fail)))

// MUST TRIGGER: an object literal stays an expression body -> Effect.failSync(() => ({ _tag: "Oops" }))
export const objectLiteralFailPipe = Effect.suspend(() => pipe({ _tag: "Oops" }, Effect.fail))

// MUST TRIGGER (report only): pipe widens the literal union to string, a failSync thunk would keep it
export const literalUnionFailPipe = Effect.suspend(() => pipe(cond ? "A" : "B", Effect.fail))

// MUST NOT TRIGGER: the fail is not the last step of the returned flow
export const failThenMapError = Effect.suspend(() => pipe(error, Effect.fail, Effect.mapError(() => "mapped")))
export const failThenMethodPipe = Effect.suspend(() => error.pipe(Effect.fail, Effect.mapError(() => "mapped")))

// MUST NOT TRIGGER: explicit type arguments on a suspend pipe step pin the error channel
export const pipeOuterTypeArguments = pipe(() => Effect.fail(error), Effect.suspend<never, PipeableError | string, never>)

// MUST NOT TRIGGER: v4 has no Effect.dieSync to suggest
export const pipeDie = pipe(() => Effect.die(defect), Effect.suspend)
export const callDiePipe = Effect.suspend(() => pipe(defect, Effect.die))
