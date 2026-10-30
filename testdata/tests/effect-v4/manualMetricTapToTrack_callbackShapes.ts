// @effect-diagnostics *:off
// @effect-diagnostics manualMetricTapToTrack:warning
import { Effect, Metric } from "effect"

declare const source: Effect.Effect<number>
declare const ambientCounter: Metric.Metric<number, Metric.CounterState<number>>
const counter = Metric.counter("counter")

// MUST TRIGGER: the parameter is the input verbatim
export const verbatim = source.pipe(Effect.tap((n) => Metric.update(counter, n)))

// MUST TRIGGER: a declared const metric is a stable reference
export const ambientMetric = source.pipe(Effect.tap(() => Metric.update(ambientCounter, 1)))

// MUST TRIGGER: a return annotation of Effect<void> is what track* returns too
export const annotatedReturn = source.pipe(Effect.tap((n): Effect.Effect<void> => Metric.update(counter, n)))

// MUST NOT TRIGGER: the callback declares a this parameter
export const thisParameter = source.pipe(Effect.tap(function(this: unknown, n: number) { return Metric.update(counter, n) }))

// MUST NOT TRIGGER: explicit type arguments on the tap
export const typeArguments = source.pipe(Effect.tap<number, void, never, never>((n) => Metric.update(counter, n)))

// MUST NOT TRIGGER: optional call of the update
export const optionalCall = source.pipe(Effect.tap((n) => Metric.update?.(counter, n)))

// MUST NOT TRIGGER: spread arguments to the update
export const spreadArguments = source.pipe(Effect.tap(() => Metric.update(...([counter, 1] as const))))

// MUST NOT TRIGGER: explicit type arguments on the update
export const typedUpdate = source.pipe(Effect.tap((n) => Metric.update<number, Metric.CounterState<number>>(counter, n)))

// MUST NOT TRIGGER: the function expression reads its own arguments
export const functionArguments = source.pipe(Effect.tap(function() { return Metric.update(counter, arguments.length) }))

// MUST NOT TRIGGER: the function expression declares a this parameter
export const functionThis = source.pipe(Effect.tap(function(this: void) { return Metric.update(counter, 1) }))

// MUST NOT TRIGGER: async callback
export const asyncCallback = source.pipe(Effect.tap(async () => Metric.update(counter, 1)))

// MUST NOT TRIGGER: generic callback
export const genericCallback = source.pipe(Effect.tap(<A>(n: A) => Metric.update(counter, 1)))

// MUST NOT TRIGGER: disabled by a next-line directive
// @effect-diagnostics-next-line manualMetricTapToTrack:off
export const disabledNextLine = source.pipe(Effect.tap((n) => Metric.update(counter, n)))
