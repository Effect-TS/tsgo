// @effect-diagnostics globalTimersInEffect:warning
import { Data, Effect } from "effect"

class ExampleError extends Data.TaggedError("ExampleError")<{}> {}

export const timeoutInSync = Effect.sync(() => setTimeout(() => {}, 100))

export const intervalInTryObject = Effect.try({
  try: () => setInterval(() => {}, 100),
  catch: () => new ExampleError()
})

export const timeoutInTryPromise = Effect.tryPromise(async () => setTimeout(() => {}, 100))

export const intervalInTryPromiseObject = Effect.tryPromise({
  try: async () => setInterval(() => {}, 100),
  catch: () => new ExampleError()
})

export const shouldNotTriggerReturnedFunction = Effect.sync(() => () => setTimeout(() => {}, 100))
