// @effect-diagnostics *:off
// @effect-diagnostics emptyCallbackToNever:warning
import { Effect, pipe } from "effect"

// MUST TRIGGER: data-first value argument
export const dataFirst = Effect.andThen(Effect.log("x"), Effect.callback<never>(() => {}))

// MUST TRIGGER: argument of a data-last step
export const dataLast = Effect.andThen(Effect.callback<never>(() => {}))(Effect.log("x"))

// MUST TRIGGER: argument of a step inside pipe(...)
export const pipeFunction = pipe(Effect.log("x"), Effect.andThen(Effect.callback<never>(() => {})))

// MUST TRIGGER: inside an Effect.fn body
export const effectFn = Effect.fn("effectFn")(function*() {
  yield* Effect.callback<never>(() => {})
})

// MUST TRIGGER: argument of an Effect.fn pipeable step
export const effectFnStep = Effect.fn("effectFnStep")(
  function*() {
    yield* Effect.log("x")
  },
  Effect.andThen(Effect.callback<never>(() => {}))
)

// MUST TRIGGER: register piped into a bare Effect.callback step (fix: Effect.never)
export const pipedRegister = pipe(() => {}, Effect.callback)

// MUST TRIGGER: register piped into an Effect.callback<never> instantiation
export const pipedRegisterNever = pipe(() => {}, Effect.callback<never>)

// MUST TRIGGER: register piped into Effect.callback with later steps (fix keeps them)
export const pipedRegisterWithSteps = pipe(
  () => {},
  Effect.callback<never>,
  Effect.onInterrupt(() => Effect.log("stopped"))
)

// MUST TRIGGER: two explicit `never` type arguments
export const twoNeverTypeArguments = Effect.callback<never, never>(() => {})

// MUST TRIGGER: parenthesized `never` type argument
export const parenthesizedNeverTypeArgument = Effect.callback<(never)>(() => {})

// MUST TRIGGER: rest parameter without a default
export const restParameter = Effect.callback<never>((..._args) => {})

// MUST NOT TRIGGER: parenthesized instantiation with a non-`never` type argument
export const parenthesizedNonNeverInstantiation = (Effect.callback<string>)(() => {})

// MUST NOT TRIGGER: alias of an instantiation with a non-`never` type argument
const stringCallback = Effect.callback<string>
export const aliasedNonNeverInstantiation = stringCallback(() => {})

// MUST NOT TRIGGER: register piped into an instantiation with a non-`never` type argument
export const pipedNonNeverInstantiation = pipe(() => {}, Effect.callback<string>)

// MUST NOT TRIGGER: non-empty register piped into Effect.callback
export const pipedNonEmptyRegister = pipe(() => {
  console.log("parked")
}, Effect.callback)

// MUST NOT TRIGGER: a destructured parameter default is evaluated on every registration
export const destructuredParameterDefault = Effect.callback<never>((_resume, { aborted = console.log("x") }) => {})

// MUST NOT TRIGGER: a `never` alias is not the `never` keyword
type Never = never
export const neverTypeAlias = Effect.callback<Never>(() => {})
