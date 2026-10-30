// @effect-diagnostics *:off
// @effect-diagnostics emptyCallbackToNever:warning
import { Effect } from "effect"
import { constVoid } from "effect/Function"

class ParkError {
  readonly _tag = "ParkError"
}
class Db {
  readonly _tag = "Db"
}

// MUST TRIGGER: empty arrow register with an explicit `never` type argument
export const blockForever = Effect.callback<never>(() => {})

// MUST TRIGGER: empty `function` expression register
export const functionExpression = Effect.callback<never>(function() {})

// MUST TRIGGER: named empty `function` expression register
export const namedFunctionExpression = Effect.callback<never>(function park() {})

// MUST TRIGGER: no type argument, success type is inferred (unknown); it never succeeds, so Effect.never fits
export const inferred = Effect.callback(() => {})

// MUST TRIGGER: every explicit type argument is `never`, so Effect.never keeps the same type
export const allNeverTypeArguments = Effect.callback<never, never, never>(() => {})

// MUST TRIGGER: success type comes from the contextual type, the annotation keeps the binding type
export const contextual: Effect.Effect<number> = Effect.callback(() => {})

// MUST TRIGGER: unused `resume` parameter, the body is still empty
export const unusedResume = Effect.callback<never>((_resume) => {})

// MUST TRIGGER: unused `resume` and `signal` parameters, the body is still empty
export const unusedResumeAndSignal = Effect.callback<never>((_resume, _signal) => {})

// MUST TRIGGER: a body holding only a comment has no statements
export const commentOnlyBody = Effect.callback<never>(() => {
  // park until interrupted
})

// MUST TRIGGER: parenthesized register literal
export const parenthesizedRegister = Effect.callback<never>((() => {}))

// MUST TRIGGER: the example from the issue, nested in a pipe
export const daemon = Effect.log("worker started").pipe(
  Effect.andThen(Effect.callback<never>(() => {}))
)

// MUST TRIGGER: yielded inside Effect.gen
export const insideGen = Effect.gen(function*() {
  yield* Effect.log("worker started")
  yield* Effect.callback<never>(() => {})
})

// MUST TRIGGER: the receiver of a further .pipe call
export const piped = Effect.callback<never>(() => {}).pipe(Effect.onInterrupt(() => Effect.log("stopped")))

// MUST TRIGGER: the replacement must keep the comments around the call
export const withComments = /* park */ Effect.callback<never>(() => {}) // until interrupted

// MUST NOT TRIGGER: already Effect.never
export const alreadyNever = Effect.never

// MUST NOT TRIGGER: explicit type arguments name non-`never` channels; Effect.never would narrow the
// binding to Effect<never> and break downstream code relying on ParkError / Db
export const explicitChannels = Effect.callback<string, ParkError, Db>(() => {})
export const catchesParkError = explicitChannels.pipe(Effect.catchTag("ParkError", () => Effect.succeed("recovered")))

// MUST NOT TRIGGER: success is `never` but the explicit error type argument is not
export const explicitError = Effect.callback<never, ParkError>(() => {})

// MUST NOT TRIGGER: register passed as an identifier (Effect.never's own internal definition)
export const identifierRegister = Effect.callback<never>(constVoid)

// MUST NOT TRIGGER: register passed through a local binding, only function literals match
const emptyRegister = () => {}
export const localIdentifierRegister = Effect.callback<never>(emptyRegister)

// MUST NOT TRIGGER: register calls resume
export const resumes = Effect.callback<number>((resume) => {
  resume(Effect.succeed(1))
})

// MUST NOT TRIGGER: register returns a cleanup effect
export const returnsCleanup = Effect.callback<never>(() => {
  return Effect.log("cleanup")
})

// MUST NOT TRIGGER: expression body returning a cleanup effect
export const expressionCleanup = Effect.callback<never>(() => Effect.log("cleanup"))

// MUST NOT TRIGGER: register uses the AbortSignal
export const usesSignal = Effect.callback<never>((_resume, signal) => {
  signal.addEventListener("abort", () => console.log("aborted"))
})

// MUST NOT TRIGGER: non-empty body that never resumes (accidental-hang territory of #483, not this rule)
export const workWithoutResume = Effect.callback<never>(() => {
  console.log("parked")
})

// MUST NOT TRIGGER: Workflow.suspend-like register that self-interrupts, not equivalent to Effect.never
export const selfInterrupting = Effect.withFiber((fiber) =>
  Effect.callback<never>(function() {
    fiber.interruptUnsafe(fiber.id)
  })
)

// MUST NOT TRIGGER: expression body returning undefined is not the empty-body form
export const expressionUndefined = Effect.callback<never>(() => undefined)

// MUST NOT TRIGGER: expression body returning void 0 is not the empty-body form
export const expressionVoid = Effect.callback<never>(() => void 0)

// MUST NOT TRIGGER: an empty statement is still a statement, only `{}` with no statements matches
export const emptyStatement = Effect.callback<never>(() => {
  ;
})

// MUST NOT TRIGGER: a parameter default is evaluated on every registration (side effect)
export const defaultParameter = Effect.callback<never>((_resume, _signal, extra = console.log("evaluated")) => {})

// MUST NOT TRIGGER: register is not a literal argument
export const spreadRegister = Effect.callback<never>(...[() => {}] as const)

// MUST NOT TRIGGER: optional call is left alone
export const optionalCall = Effect.callback?.<never>(() => {})

// MUST NOT TRIGGER: other Effect constructors taking an empty function
export const emptySync = Effect.sync(() => {})
export const neverPromise = Effect.promise<never>(() => new Promise(() => {}))
export const suspendedNever = Effect.suspend(() => Effect.never)

// MUST NOT TRIGGER: unrelated `callback` APIs
const unrelated = { callback: <A>(register: () => void): A | undefined => (register(), undefined) }
export const unrelatedCallback = unrelated.callback<never>(() => {})
export const shadowedEffect = ((Effect: typeof unrelated) => Effect.callback<never>(() => {}))(unrelated)
function callback(register: () => void) {
  register()
}
callback(() => {})

// MUST NOT TRIGGER: Effect.callback referenced but not called
export const reference = Effect.callback

// MUST NOT TRIGGER: async register is a type error (returns a Promise)
// @ts-expect-error
export const asyncRegister = Effect.callback<never>(async () => {})

// MUST NOT TRIGGER: generator register is a type error (returns a Generator)
// @ts-expect-error
export const generatorRegister = Effect.callback<never>(function*() {})
