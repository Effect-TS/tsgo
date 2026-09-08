// @effect-v3
import { Effect } from "effect"

const acquireLock: Effect.Effect<void> = Effect.log("lock acquired")
const runJob: Effect.Effect<number> = Effect.succeed(42)
const makeJob = (x: number): Effect.Effect<number> => Effect.succeed(x)

// Negative control: GOOD cases that must stay silent

// 1. Call expression body: must stay lazy
export const goodCallExpression = acquireLock.pipe(
  Effect.flatMap(() => makeJob(42))
)

// 2. Call expression constructing an Effect must stay lazy
export const goodEffectFailCall = acquireLock.pipe(
  Effect.flatMap(() => Effect.fail("error"))
)

// 3. Callback parameter is referenced
const lockWithJob: Effect.Effect<{ job: Effect.Effect<number> }> = Effect.succeed({ job: runJob })
export const goodParamReferenced = lockWithJob.pipe(
  Effect.flatMap((x) => x.job)
)

// 4. Block body
export const goodBlockBody = acquireLock.pipe(
  Effect.flatMap(() => {
    return runJob
  })
)

// 5. Already using Effect.andThen
export const goodAlreadyAndThen = acquireLock.pipe(
  Effect.andThen(runJob)
)

// 6. Unrelated object with flatMap
const unrelated = {
  flatMap: <A, B>(f: (val: A) => B): B => f(undefined as any)
}
export const goodUnrelated = unrelated.flatMap(() => runJob)
