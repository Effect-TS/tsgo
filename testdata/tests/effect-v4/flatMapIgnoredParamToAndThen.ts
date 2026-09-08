import { Effect, Exit, pipe } from "effect"

const acquireLock: Effect.Effect<void> = Effect.log("lock acquired")
const runJob: Effect.Effect<number> = Effect.succeed(42)
const exitValue: Exit.Exit<number, never> = Exit.succeed(42)
const makeJob = (x: number): Effect.Effect<number> => Effect.succeed(x)

const service = {
  job: runJob
}

// BAD: zero parameters, pre-existing effect identifier
export const badPipeable = acquireLock.pipe(
  Effect.flatMap(() => runJob)
)

// BAD: single unused parameter with underscore
export const badUnusedUnderscore = acquireLock.pipe(
  Effect.flatMap((_) => runJob)
)

// BAD: single unused named parameter
export const badUnusedNamed = acquireLock.pipe(
  Effect.flatMap((x) => runJob)
)

// BAD: pipe style
export const badPipe = pipe(
  acquireLock,
  Effect.flatMap(() => runJob)
)

// BAD: data-first style
export const badDataFirst = Effect.flatMap(acquireLock, () => runJob)

// BAD: property access body
export const badPropertyAccess = acquireLock.pipe(
  Effect.flatMap(() => service.job)
)

// BAD: Exit-typed body (an Effect subtype)
export const badExitBody = acquireLock.pipe(
  Effect.flatMap(() => exitValue)
)

// GOOD: call expression body must stay lazy (subscription-time construction)
export const goodCallExpression = acquireLock.pipe(
  Effect.flatMap(() => makeJob(42))
)

// GOOD: call expression constructing an Effect must stay lazy
export const goodEffectFailCall = acquireLock.pipe(
  Effect.flatMap(() => Effect.fail("error"))
)

// GOOD: callback parameter is referenced
const lockWithJob: Effect.Effect<{ job: Effect.Effect<number> }> = Effect.succeed({ job: runJob })
export const goodParamReferenced = lockWithJob.pipe(
  Effect.flatMap((x) => x.job)
)

// GOOD: property access on callback parameter
const effectContainer = (val: void) => ({ inner: Effect.succeed(val) })
export const goodParamPropertyAccess = acquireLock.pipe(
  Effect.map(effectContainer),
  Effect.flatMap((container) => container.inner)
)

// GOOD: block body
export const goodBlockBody = acquireLock.pipe(
  Effect.flatMap(() => {
    return runJob
  })
)

// GOOD: already using Effect.andThen
export const goodAlreadyAndThen = acquireLock.pipe(
  Effect.andThen(runJob)
)

// GOOD: unrelated object with flatMap
const unrelated = {
  flatMap: <A, B>(f: (val: A) => B): B => f(undefined as any)
}
export const goodUnrelated = unrelated.flatMap(() => runJob)
