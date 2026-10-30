// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics suspendFailToFailSync:warning
import * as Cause from "effect/Cause"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as Fx from "effect/Effect"
import { die, fail, suspend } from "effect/Effect"
import * as Exit from "effect/Exit"
import { pipe } from "effect/Function"
import * as Layer from "effect/Layer"
import * as Micro from "effect/Micro"
import * as Stream from "effect/Stream"

class ValidationError extends Data.TaggedError("ValidationError")<{
  readonly input: string
}> {}

class AppError {
  readonly _tag = "AppError"
}

declare const error: AppError
declare const defect: unknown
declare const cause: Cause.Cause<AppError>
declare const cond: boolean
declare const makeError: () => AppError
declare const makeFailure: () => Effect.Effect<never, AppError>
declare const log: (message: string) => void

// MUST TRIGGER: issue example -> Effect.failSync(() => new ValidationError({ input }))
export const rejectInput = (input: string) => Effect.suspend(() => Effect.fail(new ValidationError({ input })))

// MUST TRIGGER: identifier error value -> Effect.failSync(() => error)
export const failIdentifier = Effect.suspend(() => Effect.fail(error))

// MUST TRIGGER: side-effecting error constructor stays lazy -> Effect.failSync(() => makeError())
export const failComputed = Effect.suspend(() => Effect.fail(makeError()))

// MUST TRIGGER: block-bodied arrow with a single return -> Effect.failSync(() => error)
export const blockBody = Effect.suspend(() => {
  return Effect.fail(error)
})

// MUST TRIGGER: function-expression thunk keeps its form
// -> Effect.failSync(function() { return new Error("boom") })
export const functionExpression = Effect.suspend(function() {
  return Effect.fail(new Error("boom"))
})

// MUST TRIGGER: parenthesized inner call -> Effect.failSync(() => error)
export const parenthesized = Effect.suspend(() => (Effect.fail(error)))

// MUST TRIGGER: explicit type argument on the inner fail is carried over
// -> Effect.failSync<AppError | string>(() => error)
export const innerTypeArgument = Effect.suspend(() => Effect.fail<AppError | string>(error))

// MUST TRIGGER: namespace import of effect/Effect keeps its qualifier -> Fx.failSync(() => error)
export const namespaceImport = Fx.suspend(() => Fx.fail(error))

// MUST TRIGGER: mixed bindings that resolve to the same APIs -> Effect.failSync(() => error)
export const mixedBindings = Effect.suspend(() => Fx.fail(error))

// MUST TRIGGER: named imports resolve to the same APIs (the fix falls back to the
// file's Effect module binding -> Effect.failSync(() => error))
export const namedImports = suspend(() => fail(error))

// MUST TRIGGER: const alias of Effect.suspend keeps its identity
// (the fix falls back to the file's Effect module binding -> Effect.failSync(() => error))
const suspendAlias = Effect.suspend
export const aliasedCallee = suspendAlias(() => Effect.fail(error))

// MUST TRIGGER: report only, no fix. Effect.fail widens the literal union to string,
// while a failSync thunk keeps "A" | "B", so the rewrite would change the error type
export const literalUnion = Effect.suspend(() => Effect.fail(cond ? "A" : "B"))

// MUST TRIGGER: a single literal widens to string in both forms -> Effect.failSync(() => "not found")
export const singleLiteral = Effect.suspend(() => Effect.fail("not found"))

// MUST TRIGGER: inside generators and pipelines
export const inGen = Effect.gen(function*() {
  return yield* Effect.suspend(() => Effect.fail(error))
})
export const inFn = Effect.fn("inFn")(function*(input: string) {
  return yield* Effect.suspend(() => Effect.fail(new ValidationError({ input })))
})
export const inPipe = pipe(
  Effect.succeed(1),
  Effect.flatMap(() => Effect.suspend(() => Effect.fail(error)))
)

// MUST TRIGGER: the suspended effect is itself piped afterwards -> Effect.failSync(() => error).pipe(...)
export const pipedAfter = Effect.suspend(() => Effect.fail(error)).pipe(
  Effect.catchAll(() => Effect.succeed(0))
)

// MUST TRIGGER: only the inner suspend; the outer thunk returns a suspend, not a fail
export const nested = Effect.suspend(() => Effect.suspend(() => Effect.fail(error)))

// MUST TRIGGER: defect variant (issue example) -> Effect.dieSync(() => new Error(`bad input: ${input}`))
export const rejectAsDefect = (input: string) => Effect.suspend(() => Effect.die(new Error(`bad input: ${input}`)))

// MUST TRIGGER: defect identifier -> Effect.dieSync(() => defect)
export const dieIdentifier = Effect.suspend(() => Effect.die(defect))

// MUST TRIGGER: block-bodied defect thunk -> Effect.dieSync(() => defect)
export const dieBlockBody = Effect.suspend(() => {
  return Effect.die(defect)
})

// MUST TRIGGER: namespace and named imports for the defect variant
// -> Fx.dieSync(() => defect) / Effect.dieSync(() => defect)
export const dieNamespaceImport = Fx.suspend(() => Fx.die(defect))
export const dieNamedImports = suspend(() => die(defect))

// MUST NOT TRIGGER: already the direct APIs
export const directFailSync = Effect.failSync(() => error)
export const directFail = Effect.fail(error)
export const directDieSync = Effect.dieSync(() => defect)

// MUST NOT TRIGGER: the thunk does real conditional or lazy work
export const conditional = (input: string) =>
  Effect.suspend(() => input.length === 0 ? Effect.fail(new ValidationError({ input })) : Effect.succeed(input))
export const ifStatement = (input: string) =>
  Effect.suspend(() => {
    if (input.length === 0) {
      return Effect.fail(new ValidationError({ input }))
    }
    return Effect.succeed(input)
  })
export const priorStatement = Effect.suspend(() => {
  log("failing")
  return Effect.fail(error)
})
export const localBinding = Effect.suspend(() => {
  const e = makeError()
  return Effect.fail(e)
})

// MUST NOT TRIGGER: the failure is transformed before being returned
export const transformed = Effect.suspend(() => Effect.fail(error).pipe(Effect.mapError((e) => e._tag)))
export const delayed = Effect.suspend(() => Effect.fail(error).pipe(Effect.delay("1 second")))

// MUST TRIGGER: the thunk returns Effect.fail as a Function.pipe step -> Effect.failSync(() => error)
export const pipedBody = Effect.suspend(() => pipe(error, Effect.fail))
// MUST NOT TRIGGER: the thunk body is not a flow ending in Effect.fail
export const pointFree = Effect.suspend(makeFailure)
export const forwarded = Effect.suspend(() => makeFailure())

// MUST TRIGGER: Effect.suspend as a Function.pipe step -> pipe(() => error, Effect.failSync)
export const outerPipe = pipe(() => Effect.fail(error), Effect.suspend)

// MUST NOT TRIGGER: other failure constructors are out of scope for this rule
export const failCause = Effect.suspend(() => Effect.failCause(cause))
export const dieMessage = Effect.suspend(() => Effect.dieMessage("boom"))

// MUST NOT TRIGGER: explicit type arguments on suspend pin the error channel,
// which Effect.failSync(() => error) would narrow
export const outerTypeArguments = Effect.suspend<never, AppError | string, never>(() => Effect.fail(error))

// MUST NOT TRIGGER: a return type annotation on the thunk pins the error channel
export const annotatedThunk = Effect.suspend((): Effect.Effect<never, AppError | string> => Effect.fail(error))

// MUST NOT TRIGGER: a type assertion on the body changes the inferred error type
export const asserted = Effect.suspend(() => Effect.fail(error) as Effect.Effect<never, AppError | string>)

// MUST NOT TRIGGER: the thunk declares a parameter, so it is not a plain LazyArg thunk
export const withParameter = Effect.suspend((_?: unknown) => Effect.fail(error))

// MUST NOT TRIGGER: Exit.fail is an Effect but not Effect.fail
export const exitFail = Effect.suspend(() => Exit.fail(error))
export const exitDie = Effect.suspend(() => Exit.die(defect))

// MUST NOT TRIGGER: suspend of other modules (Stream, Layer, Micro) is not Effect.suspend
export const streamSuspend = Stream.suspend(() => Stream.fail(error))
export const layerSuspend = Layer.suspend(() => Layer.effectDiscard(Effect.fail(error)))
export const microSuspend = Micro.suspend(() => Micro.fail(error))

// MUST NOT TRIGGER: user-defined APIs with the same names
const Local = {
  suspend: <A, E>(thunk: () => Effect.Effect<A, E>) => Effect.suspend(thunk),
  fail: <E>(e: E) => Effect.fail(e),
  die: (d: unknown) => Effect.die(d)
}
export const localSuspend = Local.suspend(() => Effect.fail(error))
export const localFail = Effect.suspend(() => Local.fail(error))
export const localDie = Effect.suspend(() => Local.die(defect))
