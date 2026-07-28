// @effect-diagnostics abortControllerInEffect:suggestion
import { Data, Effect } from "effect"

class ExampleError extends Data.TaggedError("ExampleError")<{}> {}

const Controller = AbortController

export const controllerInGen = Effect.gen(function*() {
  const direct = new AbortController()
  const aliased = new Controller()
  return [direct, aliased]
})

export const controllerInFn = Effect.fn("controllerInFn")(function*() {
  return new AbortController()
})

export const controllerWithFinalizer = Effect.gen(function*() {
  const controller = new AbortController()
  yield* Effect.addFinalizer(() => Effect.sync(() => controller.abort()))
  return controller.signal
})

// Non-generator Effect thunks are outside this rule's scope.
export const controllerInSync = Effect.sync(() => new AbortController())
export const controllerInPromise = Effect.promise(async () => new AbortController())
export const controllerInCallback = Effect.callback<AbortController>((resume) => {
  const controller = new AbortController()
  resume(Effect.succeed(controller))
})
export const controllerInSuspend = Effect.suspend(() => Effect.succeed(new AbortController()))
export const controllerInTry = Effect.try(() => new AbortController())
export const controllerInTryObject = Effect.try({
  try: () => new AbortController(),
  catch: () => new ExampleError()
})
export const controllerInTryPromise = Effect.tryPromise(async () => new AbortController())
export const controllerInTryPromiseObject = Effect.tryPromise({
  try: async () => new AbortController(),
  catch: () => new ExampleError()
})

new AbortController()
export const controllerOutsideEffect = () => new AbortController()

export const controllerInNestedFunction = Effect.gen(function*() {
  return () => new AbortController()
})

export const shadowedController = Effect.gen(function*() {
  class AbortController {
    readonly signal = "local"
  }
  return new AbortController()
})
