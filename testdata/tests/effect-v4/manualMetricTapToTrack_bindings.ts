// @effect-diagnostics *:off
// @effect-diagnostics manualMetricTapToTrack:warning
import { Effect, Metric } from "effect"
import { tap } from "effect/Effect"
import { counter, update } from "effect/Metric"

const responseSize = Metric.gauge("response_size")
const requestErrors = counter("request_errors")
const ops = { tap: Effect.tap, update: Metric.update }

declare const handleRequest: Effect.Effect<number, Error>

// MUST TRIGGER: named tap import, the fix reaches the Effect module through its import
// -> Effect.trackSuccesses(responseSize)
export const namedTap = handleRequest.pipe(tap((size) => Metric.update(responseSize, size)))

// MUST TRIGGER: named update import, the fix reaches the Metric module through its import
// -> Effect.trackSuccesses(Metric.withConstantInput(requestErrors, 1))
export const namedUpdateDirect = handleRequest.pipe(Effect.tap(update(requestErrors, 1)))

// MUST TRIGGER, NO QUICKFIX: parameters shadow `Effect` and `Metric`, so no binding
// visible here reaches Effect.trackSuccesses; the fix must not reference the shadowed names
export function shadowed(Effect: number, Metric: string) {
  return handleRequest.pipe(tap(update(requestErrors, Effect + Metric.length)))
}

// MUST NOT TRIGGER: properties of a plain object are not references to the module APIs
export const objectReceivers = handleRequest.pipe(ops.tap(ops.update(requestErrors, 1)))

// MUST TRIGGER: the receivers are parameters typed as the modules, not the module
// bindings; the fix reaches the imported modules instead
// -> Effect.trackSuccesses(Metric.withConstantInput(requestErrors, 1))
export function typedReceivers(E: typeof Effect, M: typeof Metric) {
  return handleRequest.pipe(E.tap(M.update(requestErrors, 1)))
}

// MUST NOT TRIGGER: a function expression parameter default reads the callback's own arguments
export const argumentsInParameter = handleRequest.pipe(
  Effect.tap(function(size = arguments.length) {
    return Metric.update(responseSize, size)
  })
)

// MUST NOT TRIGGER: a function expression input reads the callback's own arguments,
// also through a nested arrow function
export const argumentsInInput = handleRequest.pipe(
  Effect.tap(function() {
    return Metric.update(responseSize, (() => arguments.length)())
  })
)

// MUST NOT TRIGGER: a named function expression reads its own name, in the input
// or in a parameter default
export const ownNameInInput = handleRequest.pipe(
  Effect.tap(function record(size) {
    return Metric.update(responseSize, size + record.length)
  })
)
export const ownNameInParameter = handleRequest.pipe(
  Effect.tap(function record(size = record.length) {
    return Metric.update(responseSize, size)
  })
)

// MUST NOT TRIGGER: new.target belongs to the function expression
export const newTargetInInput = handleRequest.pipe(
  Effect.tap(function(size) {
    return Metric.update(responseSize, new.target === undefined ? size : 0)
  })
)

// MUST TRIGGER: a nested non-arrow function binds its own arguments
// -> Effect.trackSuccesses(responseSize, (size) => size + (function() { ... })())
export const nestedFunctionArguments = handleRequest.pipe(
  Effect.tap(function(size) {
    return Metric.update(responseSize, size + (function() { return arguments.length })())
  })
)

// MUST TRIGGER: a named function expression that never reads its name
// -> Effect.trackSuccesses(responseSize, (size) => size * 2)
export const unusedOwnName = handleRequest.pipe(
  Effect.tap(function record(size) {
    return Metric.update(responseSize, size * 2)
  })
)

// MUST TRIGGER: an arrow callback reads the enclosing function's arguments; the
// arrow mapper stays in the same function, so it reads the same arguments
export function enclosingArguments(..._args: Array<number>) {
  return handleRequest.pipe(Effect.tap((size) => Metric.update(responseSize, size + arguments.length)))
}
