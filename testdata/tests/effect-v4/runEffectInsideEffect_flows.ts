import { Effect, pipe } from "effect"

const task = Effect.succeed(1)

export const withOptions = Effect.gen(function*() {
  yield* Effect.log("starting")
  Effect.runPromise(task, {})
})

export const inPipeMethod = Effect.gen(function*() {
  yield* Effect.log("starting")
  task.pipe(Effect.asVoid, Effect.runSync)
})

export const inPipeFunction = Effect.gen(function*() {
  yield* Effect.log("starting")
  pipe(task, Effect.runPromise)
})

export const inNestedPipe = Effect.gen(function*() {
  yield* Effect.log("starting")
  const run = () => task.pipe(Effect.runFork)
  return run
})

export const inMapCallback = Effect.succeed(1).pipe(
  Effect.map(() => Effect.runSync(task))
)

export const atEntryPoint = task.pipe(Effect.runPromise)
