// @effect-diagnostics *:off
// @effect-diagnostics sleepThenEffectToDelay:warning
import { Data, Duration, Effect, pipe, Schedule } from "effect"
import * as Fx from "effect/Effect"
import { andThen, flatMap, sleep } from "effect/Effect"
import { TestClock } from "effect/testing"

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

// MUST TRIGGER: data-first andThen with an Effect continuation -> Effect.delay(sendRequest, "1 second")
export const andThenDataFirst = Effect.andThen(Effect.sleep("1 second"), sendRequest)

// MUST TRIGGER: data-first andThen with a zero-parameter thunk
// -> Effect.delay(sendRequest, "1 second")
export const andThenDataFirstThunk = Effect.andThen(Effect.sleep("1 second"), () => sendRequest)

// MUST TRIGGER: pipe form with a flatMap thunk -> sendRequest.pipe(Effect.delay("1 second"))
export const flatMapPipeable = Effect.sleep("1 second").pipe(Effect.flatMap(() => sendRequest))

// MUST TRIGGER: pipe form with an andThen thunk -> sendRequest.pipe(Effect.delay("1 second"))
export const andThenPipeableThunk = Effect.sleep("1 second").pipe(Effect.andThen(() => sendRequest))

// MUST TRIGGER: free pipe function, the sequencer is the stage right after the sleep
// -> pipe(sendRequest, Effect.delay("1 second"))
export const freePipe = pipe(Effect.sleep("1 second"), Effect.andThen(sendRequest))

// MUST TRIGGER: data-last call applied to the sleep -> Effect.delay(sendRequest, "1 second")
export const dataLastApplied = Effect.andThen(sendRequest)(Effect.sleep("1 second"))

// MUST TRIGGER: only the first pipe stage is rewritten, the remaining stages are kept
// -> fetchUser.pipe(Effect.delay(retryAfter), Effect.map((user) => user.id), Effect.forkDetach)
export const moreStages = Effect.sleep(retryAfter).pipe(
  Effect.andThen(fetchUser),
  Effect.map((user) => user.id),
  Effect.forkDetach
)

// MUST TRIGGER: HttpClient.ts:1111 shape, the thunk constructs the effect after the sleep
// (the continuation is a call, so a fix must keep it lazy:
// -> Effect.delay(Effect.suspend(() => loop(request)), retryAfter))
export const thunkedCall = Effect.flatMap(Effect.sleep(retryAfter), () => loop(request))

// MUST TRIGGER: HttpClient.ts:1196 shape, a zero-parameter function reference as continuation.
// Report only: `runAdaptive` may be bound to a non-arrow function, which could tell the call
// from Effect.suspend(runAdaptive) apart through `arguments` or `this`
export const functionReference = Effect.flatMap(Effect.sleep(retryAfter), runAdaptive)

// MUST TRIGGER: zero-parameter function reference through andThen (report only, as above)
export const andThenFunctionReference = Effect.sleep(retryAfter).pipe(Effect.andThen(runAdaptive))

// MUST TRIGGER: sleep-then-fail, the timeout arm of a race
// -> Effect.fail(new TimeoutError({ message: "timed out" })).pipe(Effect.delay("2 seconds"))
export const sleepThenFail = Effect.race(
  sendRequest,
  Effect.sleep("2 seconds").pipe(Effect.andThen(Effect.fail(new TimeoutError({ message: "timed out" }))))
)

// MUST TRIGGER: the continuation keeps its error and requirement channels
export const keepsChannels = Effect.flatMap(Effect.sleep(Duration.seconds(5)), () => fetchUser)

// MUST TRIGGER: block-bodied callback with a single return
export const blockBody = Effect.flatMap(Effect.sleep("1 second"), () => {
  return sendRequest
})

// MUST TRIGGER: block-bodied callback with side effects (a fix must keep it lazy with Effect.suspend)
export const blockBodyWithSideEffects = Effect.flatMap(Effect.sleep("1 second"), () => {
  log("retrying")
  return sendRequest
})

// MUST TRIGGER: function-expression callback
export const functionExpression = Effect.flatMap(Effect.sleep("1 second"), function() {
  return sendRequest
})

// MUST TRIGGER: conditional continuation built inside the thunk
export const conditionalThunk = Effect.flatMap(
  Effect.sleep("1 second"),
  () => cond ? sendRequest : Effect.void
)

// MUST TRIGGER: the callback declares the void parameter but never reads it
export const ignoredParameter = Effect.flatMap(Effect.sleep("1 second"), (_) => sendRequest)

// MUST TRIGGER: the duration is not inspected, any duration expression matches
export const zeroDuration = Effect.sleep(0).pipe(Effect.andThen(sendRequest))
export const durationConstructor = Effect.sleep(Duration.millis(250)).pipe(Effect.andThen(sendRequest))

// MUST TRIGGER: namespace import of effect/Effect -> Fx.delay(sendRequest, "1 second")
export const namespaceImport = Fx.flatMap(Fx.sleep("1 second"), () => sendRequest)

// MUST TRIGGER: named imports resolve to the same APIs
export const namedImports = flatMap(sleep("1 second"), () => sendRequest)
export const namedImportsPipe = sleep("1 second").pipe(andThen(sendRequest))

// MUST TRIGGER: const alias of Effect.sleep keeps its identity
const sleepAlias = Effect.sleep
export const aliasedSleep = Effect.andThen(sleepAlias("1 second"), sendRequest)

// MUST TRIGGER: parenthesized sleep call as first operand
export const parenthesizedSleep = Effect.andThen((Effect.sleep("1 second")), sendRequest)

// MUST TRIGGER: nested inside generators and Effect.fn bodies
export const inGen = Effect.gen(function*() {
  yield* Effect.sleep("1 second").pipe(Effect.andThen(sendRequest))
})
export const inFn = Effect.fn("inFn")(function*(input: Request) {
  return yield* Effect.flatMap(Effect.sleep(retryAfter), () => loop(input))
})

// MUST TRIGGER: inner sleep-then pair nested in a larger pipeline
export const nestedInPipeline = sendRequest.pipe(
  Effect.andThen(Effect.sleep("1 second").pipe(Effect.andThen(sendRequest))),
  Effect.forever
)

// MUST NOT TRIGGER: already uses Effect.delay
export const alreadyDelayed = sendRequest.pipe(Effect.delay("1 second"))
export const alreadyDelayedDataFirst = Effect.delay(sendRequest, "100 millis")

// MUST NOT TRIGGER: sequential sleep as a standalone statement in a generator is idiomatic
export const sequentialGen = Effect.gen(function*() {
  yield* Effect.sleep("1 second")
  yield* sendRequest
})
export const sequentialFn = Effect.fn("sequentialFn")(function*() {
  yield* Effect.sleep("1 second")
  return yield* fetchUser
})

// MUST NOT TRIGGER: the first pipe stage is not a sequencer
export const withLiveFirst = Effect.sleep("1 second").pipe(TestClock.withLive, Effect.andThen(sendRequest))
export const uninterruptibleFirst = Effect.sleep("1 second").pipe(
  Effect.uninterruptible,
  Effect.andThen(sendRequest)
)
export const scheduleFirst = Effect.sleep("1 second").pipe(Effect.schedule(Schedule.recurs(3)))
export const freePipeNotFirst = pipe(Effect.sleep("1 second"), Effect.uninterruptible, Effect.andThen(sendRequest))

// MUST NOT TRIGGER: the sleep is sequenced after the effect, not before it
export const sleepAfter = sendRequest.pipe(Effect.andThen(Effect.sleep("1 second")))
export const sleepAfterDataFirst = Effect.andThen(sendRequest, Effect.sleep("1 second"))
export const sleepAfterFlatMap = Effect.flatMap(sendRequest, () => Effect.sleep("1 second"))

// MUST NOT TRIGGER: the combinator does not replace the sleep result with the continuation's
export const tapAfterSleep = Effect.sleep("1 second").pipe(Effect.tap(sendRequest))
export const zipWithSleep = Effect.zip(Effect.sleep("1 second"), fetchUser)
export const asAfterSleep = Effect.sleep("1 second").pipe(Effect.as("ready"))
export const mapAfterSleep = Effect.sleep("1 second").pipe(Effect.map(() => 1))

// MUST NOT TRIGGER: the callback reads the sleep result
export const usesParameter = Effect.flatMap(Effect.sleep("1 second"), (value) => logValue(value))
export const usesParameterPipe = Effect.sleep("1 second").pipe(Effect.andThen((value) => logValue(value)))

// MUST NOT TRIGGER: a function reference that takes a parameter could read the sleep result
export const parameterizedReference = Effect.flatMap(Effect.sleep("1 second"), logValue)

// MUST NOT TRIGGER: the first operand is not a direct Effect.sleep call
const wait = Effect.sleep("1 second")
export const sleepBinding = wait.pipe(Effect.andThen(sendRequest))
export const sleepBindingDataFirst = Effect.andThen(wait, sendRequest)
export const sleepWithStage = Effect.andThen(Effect.sleep("1 second").pipe(Effect.uninterruptible), sendRequest)
export const delayedVoid = Effect.andThen(Effect.void.pipe(Effect.delay("1 second")), sendRequest)

// MUST NOT TRIGGER: the pipe call has no stages or spreads them
export const noStages = Effect.sleep("1 second").pipe()
declare const stages: ReadonlyArray<(self: Effect.Effect<void>) => Effect.Effect<void>>
export const spreadStages = pipe(Effect.sleep("1 second"), ...(stages as []))

// MUST NOT TRIGGER: unrelated sleep and sequencing APIs
const unrelated = {
  sleep: (_duration: string): Effect.Effect<void> => Effect.void,
  andThen: <A, B>(_first: A, second: B): B => second
}
export const unrelatedSleep = unrelated.sleep("1 second").pipe(Effect.andThen(sendRequest))
export const unrelatedAndThen = unrelated.andThen(Effect.sleep("1 second"), sendRequest)
export const shadowedEffect = ((Effect: typeof unrelated) => Effect.andThen(Effect.sleep("1 second"), sendRequest))(
  unrelated
)
