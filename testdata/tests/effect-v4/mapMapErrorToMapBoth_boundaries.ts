// @effect-diagnostics *:off
// @effect-diagnostics mapMapErrorToMapBoth:warning
import { Effect, pipe } from "effect"

declare const source: Effect.Effect<number, string>
let multiplier = 1
const makeSuccess = () => {
  const captured = multiplier
  return (n: number) => n * captured
}
// A user-defined pipe step that runs code when applied
const record = <A, E, R>(self: Effect.Effect<A, E, R>) => {
  multiplier = 2
  return self
}

// MUST TRIGGER (no quickfix): `record` runs before the second pipe is evaluated, so moving
// makeSuccess() into the second pipe would evaluate it after `record` (result 6 instead of 3)
export const precedingStepRuns = Effect.succeed(3)
  .pipe(record, Effect.map(makeSuccess()))
  .pipe(Effect.mapError(String))

// MUST TRIGGER: same shape with a function literal callback; the quickfix is offered
export const precedingStepLiteral = Effect.succeed(3)
  .pipe(record, Effect.map((n) => n * multiplier))
  .pipe(Effect.mapError(String))

// MUST TRIGGER: the pair starts the first pipe, so nothing runs between the callbacks;
// the quickfix is offered for a non-literal callback
export const firstStepOfPipe = Effect.succeed(3)
  .pipe(Effect.map(makeSuccess()))
  .pipe(Effect.mapError(String), record)

// MUST TRIGGER (no quickfix): explicit type arguments on an empty .pipe() between the steps;
// removing Effect.map changes its input type
export const emptyPipeTypeArguments = Effect.mapError(
  Effect.map(source, (n) => String(n)).pipe<Effect.Effect<string, string>>(),
  (message) => message.length
)

// MUST TRIGGER (no quickfix): explicit type arguments on the pipe call that holds the first step
export const firstPipeTypeArguments = Effect.mapError(
  pipe<Effect.Effect<number, string>, Effect.Effect<string, string>>(source, Effect.map((n) => String(n))),
  (message) => message.length
)

// MUST TRIGGER: an empty .pipe() without type arguments between the steps; the quickfix is offered
export const emptyPipe = Effect.mapError(
  Effect.map(source, (n) => String(n)).pipe(),
  (message) => message.length
)

// MUST TRIGGER (no quickfix): an eager step before the pair runs before makeSuccess() is moved
export const precedingEagerStep = Effect.succeed(3).pipe(
  Effect.mapEager((n) => {
    multiplier = 2
    return n
  }),
  Effect.map(makeSuccess())
).pipe(Effect.mapError(String))

declare const makeTap: () => <A, E, R>(self: Effect.Effect<A, E, R>) => Effect.Effect<A, E, R>

// MUST TRIGGER (no quickfix): the eager first step runs before makeTap() is evaluated; the
// merged Effect.mapBothEager would run it after
export const eagerWorkBeforeLaterArgument = Effect.succeed(3)
  .pipe(Effect.mapEager((n) => n * multiplier))
  .pipe(Effect.mapErrorEager((message) => message), makeTap())

// MUST TRIGGER: function literal callbacks and the second step ends its pipe, so nothing
// observable runs between the first step's work and the merged step; the quickfix is offered
export const eagerWorkLastArgument = Effect.succeed(3)
  .pipe(record, Effect.mapEager((n) => n * multiplier))
  .pipe(Effect.mapErrorEager((message) => message))
