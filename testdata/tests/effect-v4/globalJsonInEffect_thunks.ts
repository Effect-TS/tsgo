// @effect-diagnostics globalJsonInEffect:warning
import { Data, Effect } from "effect"

class ExampleError extends Data.TaggedError("ExampleError")<{}> {}

export const parseInSync = Effect.sync(() => JSON.parse("{}"))

export const parseInTryObject = Effect.try({
  try: () => JSON.parse("{}"),
  catch: () => new ExampleError()
})

export const stringifyInTryObject = Effect.try({
  try: () => JSON.stringify({ name: "Alice" }),
  catch: () => new ExampleError()
})

export const parseInTryObjectBlock = Effect.try({
  try: () => {
    const parsed = JSON.parse("{}")
    return parsed
  },
  catch: () => new ExampleError()
})

export const parseInTryPromise = Effect.tryPromise(async () => JSON.parse("{}"))

export const parseInTryPromiseObject = Effect.tryPromise({
  try: async () => JSON.parse("{}"),
  catch: () => new ExampleError()
})

export const shouldNotTriggerReturnedFunction = Effect.sync(() => () => JSON.parse("{}"))
