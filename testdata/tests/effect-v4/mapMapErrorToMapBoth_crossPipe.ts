// @effect-diagnostics *:off
// @effect-diagnostics mapMapErrorToMapBoth:warning
import { Data, Effect, pipe } from "effect"

class TaskError extends Data.TaggedError("TaskError")<{
  readonly message: string
}> {}

declare const fetchCount: Effect.Effect<number, string>

// MUST TRIGGER: the first .pipe(...) is left empty by the fix and collapses to its subject
export const emptiedPipe = fetchCount
  .pipe(Effect.map((n) => n > 0))
  .pipe(Effect.mapError((message) => new TaskError({ message })))

// MUST TRIGGER: reverse order across chained .pipe(...) calls
export const emptiedPipeReverse = fetchCount
  .pipe(Effect.mapError((message) => new TaskError({ message })))
  .pipe(Effect.map((n) => n > 0))

// MUST TRIGGER: other steps on both sides of the pipe boundary are kept
export const stepsOnBothSides = fetchCount
  .pipe(
    Effect.tap((n) => Effect.log(n)),
    Effect.map((n) => n > 0)
  )
  .pipe(
    Effect.mapError((message) => new TaskError({ message })),
    Effect.asVoid
  )

// MUST TRIGGER: nested function pipe; the inner pipe(...) collapses to its subject
export const nestedFunctionPipe = pipe(
  pipe(fetchCount, Effect.map((n) => n > 0)),
  Effect.mapError((message) => new TaskError({ message }))
)

// MUST TRIGGER: data-first inner call, pipeable outer step
export const dataFirstThenPipeable = Effect.map(fetchCount, (n) => n > 0).pipe(
  Effect.mapError((message) => new TaskError({ message }))
)

// MUST TRIGGER: pipeable inner steps, data-first outer call; the earlier step is kept
export const pipeableThenDataFirst = Effect.mapError(
  fetchCount.pipe(
    Effect.tap((n) => Effect.log(n)),
    Effect.map((n) => n > 0)
  ),
  (message) => new TaskError({ message })
)

// MUST TRIGGER: curried applications
export const curried = Effect.mapError((message: string) => new TaskError({ message }))(
  Effect.map((n: number) => n > 0)(fetchCount)
)

// MUST TRIGGER: Effect.fnUntraced pipeline steps
export const untracedPipeline = Effect.fnUntraced(
  function*(n: number) {
    return yield* Effect.succeed(n)
  },
  Effect.map((n) => n > 0),
  Effect.mapError(() => new TaskError({ message: "never" }))
)

// MUST NOT TRIGGER: a step separates the pair across the pipe boundary
export const separatedAcrossPipes = fetchCount
  .pipe(
    Effect.map((n) => n > 0),
    Effect.tap((positive) => Effect.log(positive))
  )
  .pipe(Effect.mapError((message) => new TaskError({ message })))

let multiplier = 1
const makeErrorMapper = () => {
  multiplier = 2
  return (message: string) => new TaskError({ message })
}

// MUST TRIGGER (no quickfix): eager pair across chained .pipe(...) calls; Effect.mapEager runs
// its callback on the resolved effect before makeErrorMapper() is evaluated, which
// Effect.mapBothEager would reorder
export const eagerCrossPipeCall = Effect.succeed(3)
  .pipe(Effect.mapEager((n) => n * multiplier))
  .pipe(Effect.mapErrorEager(makeErrorMapper()))

// MUST TRIGGER (no quickfix): eager data-first inner call, pipeable outer step with a call
export const eagerDataFirstThenPipeableCall = Effect.mapEager(Effect.succeed(3), (n) => n * multiplier).pipe(
  Effect.mapErrorEager(makeErrorMapper())
)

// MUST TRIGGER: eager pair across chained .pipe(...) calls with a function literal; the fix is offered
export const eagerCrossPipeLiteral = Effect.succeed(3)
  .pipe(Effect.mapEager((n) => n * multiplier))
  .pipe(Effect.mapErrorEager((message) => new TaskError({ message })))
