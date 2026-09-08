// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics sleepThenEffectToDelay:suggestion
import { Effect, pipe } from "effect"

declare const sendRequest: Effect.Effect<void>

// BAD: pipe form with andThen
export const badPipeAndThen = Effect.sleep("1 second").pipe(
  Effect.andThen(sendRequest)
)

// BAD: pipe form with flatMap (0 params)
export const badPipeFlatMap = Effect.sleep("1 second").pipe(
  Effect.flatMap(() => sendRequest)
)

// BAD: pipe form with flatMap (unused param _)
export const badPipeFlatMapUnderscore = Effect.sleep("1 second").pipe(
  Effect.flatMap((_) => sendRequest)
)

// BAD: pipe form with flatMap (unused named param)
export const badPipeFlatMapUnusedNamed = Effect.sleep("1 second").pipe(
  Effect.flatMap((_unused) => sendRequest)
)

// BAD: pipe helper function with andThen
export const badPipeHelperAndThen = pipe(
  Effect.sleep("1 second"),
  Effect.andThen(sendRequest)
)

// BAD: pipe helper function with flatMap
export const badPipeHelperFlatMap = pipe(
  Effect.sleep("1 second"),
  Effect.flatMap(() => sendRequest)
)

// BAD: pipe with sleep transformation step (flow.Transformations[0])
export const badPipeSleepStep = pipe(
  "1 second",
  Effect.sleep,
  Effect.andThen(sendRequest)
)

// BAD: data-first andThen
export const badDataFirstAndThen = Effect.andThen(
  Effect.sleep("1 second"),
  sendRequest
)

// BAD: data-first flatMap
export const badDataFirstFlatMap = Effect.flatMap(
  Effect.sleep("100 millis"),
  () => sendRequest
)

// GOOD: Effect.delay in pipe form
export const goodDelayPipe = sendRequest.pipe(Effect.delay("1 second"))

// GOOD: Effect.delay data-first form
export const goodDelayDataFirst = Effect.delay(sendRequest, "100 millis")

// GOOD: standalone yield* sleep in Effect.gen
export const goodGenStandalone = Effect.gen(function* () {
  yield* Effect.sleep("1 second")
  return yield* sendRequest
})

// GOOD: non-sequencer first stage (withLive)
declare const withLive: <A, E, R>(effect: Effect.Effect<A, E, R>) => Effect.Effect<A, E, R>
export const goodTestClockFirstStage = Effect.sleep("1 second").pipe(
  withLive,
  Effect.andThen(sendRequest)
)

// GOOD: non-sequencer first stage (schedule)
declare const schedule: (effect: Effect.Effect<void>) => Effect.Effect<void>
export const goodScheduleFirstStage = Effect.sleep("1 second").pipe(
  schedule,
  Effect.andThen(sendRequest)
)

// GOOD: unrelated sleep function
declare const unrelatedSleep: (d: string) => { pipe: (f: any) => any }
export const goodUnrelatedSleep = unrelatedSleep("1 second").pipe(
  Effect.andThen(sendRequest)
)

// GOOD: param-using callback in pipe flatMap
export const goodParamUsingCallback = Effect.sleep("1 second").pipe(
  Effect.flatMap((val) => Effect.succeed(val))
)

// GOOD: param-using callback in data-first flatMap
export const goodDataFirstParamUsing = Effect.flatMap(
  Effect.sleep("1 second"),
  (val) => Effect.succeed(val)
)


// GOOD: standalone sleep without sequencer
export const goodStandaloneSleep = Effect.sleep("1 second")
