// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics sleepThenEffectToDelay:warning
import { Data, Duration, Effect, pipe, Schedule } from "effect"
import * as Fx from "effect/Effect"
import { andThen, flatMap, sleep, zipRight } from "effect/Effect"

class TimeoutError extends Data.TaggedError("TimeoutError")<{ readonly message: string }> {}

interface Request {
  readonly url: string
}

declare const sendRequest: Effect.Effect<void>
declare const fetchUser: Effect.Effect<{ readonly id: string }, TimeoutError, Request>
declare const retryAfter: Duration.Duration
declare const cond: boolean
declare const loop: (request: Request) => Effect.Effect<string, TimeoutError>
declare const runAdaptive: () => Effect.Effect<number>
declare const logValue: (value: unknown) => Effect.Effect<void>
declare const fetchPromise: () => Promise<number>
declare const request: Request
declare const log: (message: string) => void

// MUST TRIGGER: issue example, pipe form with an Effect continuation as the first stage
// -> sendRequest.pipe(Effect.delay("1 second"))
export const throttled = Effect.sleep("1 second").pipe(
  Effect.andThen(sendRequest)
)

// MUST TRIGGER: issue example, data-first flatMap with a zero-parameter thunk
// -> Effect.delay(sendRequest, "100 millis")
export const retryLater = Effect.flatMap(Effect.sleep("100 millis"), () => sendRequest)

// MUST TRIGGER: data-first zipRight (v3 only) -> Effect.delay(sendRequest, "1 second")
export const zipRightDataFirst = Effect.zipRight(Effect.sleep("1 second"), sendRequest)

// MUST TRIGGER: pipe form zipRight (v3 only) -> sendRequest.pipe(Effect.delay("1 second"))
export const zipRightPipeable = Effect.sleep("1 second").pipe(Effect.zipRight(sendRequest))

// MUST TRIGGER: data-first andThen with an Effect continuation -> Effect.delay(sendRequest, "1 second")
export const andThenDataFirst = Effect.andThen(Effect.sleep("1 second"), sendRequest)

// MUST TRIGGER: data-first andThen with a zero-parameter thunk returning an Effect
export const andThenDataFirstThunk = Effect.andThen(Effect.sleep("1 second"), () => sendRequest)

// MUST TRIGGER: pipe form with a flatMap thunk -> sendRequest.pipe(Effect.delay("1 second"))
export const flatMapPipeable = Effect.sleep("1 second").pipe(Effect.flatMap(() => sendRequest))

// MUST TRIGGER: pipe form with an andThen thunk returning an Effect
export const andThenPipeableThunk = Effect.sleep("1 second").pipe(Effect.andThen(() => sendRequest))

// MUST TRIGGER: free pipe function, the sequencer is the stage right after the sleep
// -> pipe(sendRequest, Effect.delay("1 second"))
export const freePipe = pipe(Effect.sleep("1 second"), Effect.zipRight(sendRequest))

// MUST TRIGGER: data-last call applied to the sleep -> Effect.delay(sendRequest, "1 second")
export const dataLastApplied = Effect.zipRight(sendRequest)(Effect.sleep("1 second"))

// MUST TRIGGER: only the first pipe stage is rewritten, the remaining stages are kept
// -> fetchUser.pipe(Effect.delay(retryAfter), Effect.map((user) => user.id), Effect.forkDaemon)
export const moreStages = Effect.sleep(retryAfter).pipe(
  Effect.andThen(fetchUser),
  Effect.map((user) => user.id),
  Effect.forkDaemon
)

// MUST TRIGGER: the thunk constructs the effect after the sleep
// (a fix must keep it lazy: -> Effect.delay(Effect.suspend(() => loop(request)), retryAfter))
export const thunkedCall = Effect.flatMap(Effect.sleep(retryAfter), () => loop(request))

// MUST TRIGGER: a zero-parameter function reference as continuation.
// Report only: `runAdaptive` may be bound to a non-arrow function, which could tell the call
// from Effect.suspend(runAdaptive) apart through `arguments` or `this`
export const functionReference = Effect.flatMap(Effect.sleep(retryAfter), runAdaptive)

// MUST TRIGGER: sleep-then-fail, the timeout arm of a race
export const sleepThenFail = Effect.race(
  sendRequest,
  Effect.sleep("2 seconds").pipe(Effect.zipRight(Effect.fail(new TimeoutError({ message: "timed out" }))))
)

// MUST TRIGGER: the continuation keeps its error and requirement channels
export const keepsChannels = Effect.flatMap(Effect.sleep(Duration.seconds(5)), () => fetchUser)

// MUST TRIGGER: block-bodied callback with side effects (a fix must keep it lazy with Effect.suspend)
export const blockBodyWithSideEffects = Effect.flatMap(Effect.sleep("1 second"), () => {
  log("retrying")
  return sendRequest
})

// MUST TRIGGER: conditional continuation built inside the thunk
export const conditionalThunk = Effect.flatMap(
  Effect.sleep("1 second"),
  () => cond ? sendRequest : Effect.void
)

// MUST TRIGGER: the callback declares the void parameter but never reads it
export const ignoredParameter = Effect.flatMap(Effect.sleep("1 second"), (_) => sendRequest)

// MUST TRIGGER: namespace import of effect/Effect -> Fx.delay(sendRequest, "1 second")
export const namespaceImport = Fx.zipRight(Fx.sleep("1 second"), sendRequest)

// MUST TRIGGER: named imports resolve to the same APIs
export const namedImports = flatMap(sleep("1 second"), () => sendRequest)
export const namedImportsPipe = sleep("1 second").pipe(andThen(sendRequest))
export const namedZipRight = zipRight(sleep("1 second"), sendRequest)

// MUST TRIGGER: nested inside generators
export const inGen = Effect.gen(function*() {
  yield* Effect.sleep("1 second").pipe(Effect.andThen(sendRequest))
})

// MUST NOT TRIGGER: already uses Effect.delay
export const alreadyDelayed = sendRequest.pipe(Effect.delay("1 second"))
export const alreadyDelayedDataFirst = Effect.delay(sendRequest, "100 millis")

// MUST NOT TRIGGER: sequential sleep as a standalone statement in a generator is idiomatic
export const sequentialGen = Effect.gen(function*() {
  yield* Effect.sleep("1 second")
  yield* sendRequest
})

// MUST NOT TRIGGER: v3 andThen also accepts plain values, value thunks and promise thunks,
// none of which is an Effect continuation that Effect.delay could take
export const andThenValue = Effect.sleep("1 second").pipe(Effect.andThen(42))
export const andThenValueThunk = Effect.sleep("1 second").pipe(Effect.andThen(() => 42))
export const andThenPromiseThunk = Effect.andThen(Effect.sleep("1 second"), () => fetchPromise())
export const andThenPromiseReference = Effect.andThen(Effect.sleep("1 second"), fetchPromise)

// MUST NOT TRIGGER: zipRight options can run both effects concurrently, unlike Effect.delay
export const zipRightConcurrent = Effect.zipRight(Effect.sleep("1 second"), sendRequest, { concurrent: true })
export const zipRightConcurrentPipe = Effect.sleep("1 second").pipe(
  Effect.zipRight(sendRequest, { concurrent: true })
)

// MUST NOT TRIGGER: the first pipe stage is not a sequencer
export const uninterruptibleFirst = Effect.sleep("1 second").pipe(
  Effect.uninterruptible,
  Effect.zipRight(sendRequest)
)
export const scheduleFirst = Effect.sleep("1 second").pipe(Effect.schedule(Schedule.recurs(3)))

// MUST NOT TRIGGER: the sleep is sequenced after the effect, not before it
export const sleepAfter = sendRequest.pipe(Effect.zipRight(Effect.sleep("1 second")))
export const sleepAfterDataFirst = Effect.andThen(sendRequest, Effect.sleep("1 second"))

// MUST NOT TRIGGER: the combinator does not replace the sleep result with the continuation's
export const zipLeftAfterSleep = Effect.zipLeft(Effect.sleep("1 second"), sendRequest)
export const tapAfterSleep = Effect.sleep("1 second").pipe(Effect.tap(sendRequest))
export const zipWithSleep = Effect.zip(Effect.sleep("1 second"), fetchUser)
export const asAfterSleep = Effect.sleep("1 second").pipe(Effect.as("ready"))

// MUST NOT TRIGGER: the callback reads the sleep result
export const usesParameter = Effect.flatMap(Effect.sleep("1 second"), (value) => logValue(value))

// MUST NOT TRIGGER: a function reference that takes a parameter could read the sleep result
export const parameterizedReference = Effect.flatMap(Effect.sleep("1 second"), logValue)

// MUST NOT TRIGGER: the first operand is not a direct Effect.sleep call
const wait = Effect.sleep("1 second")
export const sleepBinding = wait.pipe(Effect.zipRight(sendRequest))
export const sleepWithStage = Effect.zipRight(Effect.sleep("1 second").pipe(Effect.uninterruptible), sendRequest)

// MUST NOT TRIGGER: unrelated sleep and sequencing APIs
const unrelated = {
  sleep: (_duration: string): Effect.Effect<void> => Effect.void,
  zipRight: <A, B>(_first: A, second: B): B => second
}
export const unrelatedSleep = unrelated.sleep("1 second").pipe(Effect.zipRight(sendRequest))
export const unrelatedZipRight = unrelated.zipRight(Effect.sleep("1 second"), sendRequest)
