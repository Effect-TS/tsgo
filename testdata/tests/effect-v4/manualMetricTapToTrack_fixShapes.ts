// @filename: forms.ts
// @effect-diagnostics *:off
// @effect-diagnostics manualMetricTapToTrack:warning
import { Effect, Metric } from "effect"

const responseSize = Metric.gauge("response_size")
const successes = Metric.counter("successes")
const userCount = Metric.gauge("user_count")
const calls = Metric.gauge("calls")
const sizes = Metric.gauge("sizes")
const requestErrors = Metric.counter("request_errors")
const updateAlias = Metric.update

declare const source: Effect.Effect<number>
declare const users: Effect.Effect<ReadonlyArray<string>>
declare const failing: Effect.Effect<number, Error>

// MUST TRIGGER: data-first callback -> Effect.trackSuccesses(source, responseSize)
export const dataFirstCallback = Effect.tap(source, (size) => Metric.update(responseSize, size))

// MUST TRIGGER: data-first direct effect
// -> Effect.trackSuccesses(source, Metric.withConstantInput(successes, 1))
export const dataFirstDirect = Effect.tap(source, Metric.update(successes, 1))

// MUST TRIGGER: an aliased update in the direct form; the fix reaches the Metric module
// -> Effect.trackSuccesses(Metric.withConstantInput(successes, 1))
export const directUpdateAlias = source.pipe(Effect.tap(updateAlias(successes, 1)))

// MUST TRIGGER: destructured parameter -> Effect.trackSuccesses(userCount, ({ length }) => length)
export const destructuredMapper = users.pipe(Effect.tap(({ length }) => Metric.update(userCount, length)))

// MUST TRIGGER: rest parameter
// -> Effect.trackSuccesses(calls, (...args: Array<number>) => args.length)
export const restParameterMapper = source.pipe(Effect.tap((...args: Array<number>) => Metric.update(calls, args.length)))

// MUST TRIGGER: default parameter -> Effect.trackSuccesses(sizes, (size = 0) => size)
export const defaultParameterMapper = source.pipe(Effect.tap((size = 0) => Metric.update(sizes, size)))

// MUST TRIGGER: pipeable argument of Effect.fn -> Effect.trackSuccesses(responseSize)
export const handler = Effect.fn("handler")(
  function*(id: string) {
    return id.length
  },
  Effect.tap((size) => Metric.update(responseSize, size))
)

// MUST TRIGGER: data-last form applied directly
// -> Effect.trackErrors(requestErrors, () => 1)(failing)
export const dataLastApplied = Effect.tapError(() => Metric.update(requestErrors, 1))(failing)

// @filename: tapAlias.ts
// @effect-diagnostics *:off
// @effect-diagnostics manualMetricTapToTrack:warning
import * as Fx from "effect/Effect"
import { Metric } from "effect"

const responseSize = Metric.gauge("response_size")
const tapAlias = Fx.tap

declare const source: Fx.Effect<number>

// MUST TRIGGER: an aliased tap; the fix reaches the Effect module through its namespace import
// -> Fx.trackSuccesses(responseSize)
export const value = source.pipe(tapAlias((size) => Metric.update(responseSize, size)))

// @filename: namespaceMetric.ts
// @effect-diagnostics *:off
// @effect-diagnostics manualMetricTapToTrack:warning
import { Effect } from "effect"
import * as M from "effect/Metric"

const successes = M.counter("successes")

declare const source: Effect.Effect<number>

// MUST TRIGGER: direct form through a namespace import of Metric
// -> Effect.trackSuccesses(M.withConstantInput(successes, 1))
export const value = source.pipe(Effect.tap(M.update(successes, 1)))

// @filename: memberImports.ts
// @effect-diagnostics *:off
// @effect-diagnostics manualMetricTapToTrack:warning
import type { Effect } from "effect"
import { tap, trackSuccesses } from "effect/Effect"
import { counter, update, withConstantInput } from "effect/Metric"

const successes = counter("successes")

declare const source: Effect.Effect<number>

// MUST TRIGGER: only direct member imports reach the target APIs
// -> trackSuccesses(withConstantInput(successes, 1))
export const value = source.pipe(tap(update(successes, 1)))

// @filename: noMetricTarget.ts
// @effect-diagnostics *:off
// @effect-diagnostics manualMetricTapToTrack:warning
import { Effect } from "effect"
import { counter, update } from "effect/Metric"

const successes = counter("successes")

declare const source: Effect.Effect<number>

// MUST TRIGGER, NO QUICKFIX: Effect.trackSuccesses is reachable but Metric.withConstantInput is not
export const value = source.pipe(Effect.tap(update(successes, 1)))
