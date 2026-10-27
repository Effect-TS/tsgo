// @effect-diagnostics *:off
// @effect-diagnostics manualMetricTapToTrack:warning
import type { Effect } from "effect"
import { tap, tapError } from "effect/Effect"
import { counter, gauge, update } from "effect/Metric"

const responseSize = gauge("response_size")
const requestErrors = counter("request_errors")

declare const handleRequest: Effect.Effect<number, Error>

// MUST TRIGGER, NO QUICKFIX: only a named tap import is in scope; `Effect` is a
// type-only import, so no binding reaches Effect.trackSuccesses
export const namedTap = handleRequest.pipe(tap((size) => update(responseSize, size)))

// MUST TRIGGER, NO QUICKFIX: no binding reaches Effect.trackErrors
export const namedTapError = tapError(handleRequest, () => update(requestErrors, 1))

// MUST TRIGGER, NO QUICKFIX: neither Effect.trackSuccesses nor Metric.withConstantInput is reachable
export const namedTapDirect = handleRequest.pipe(tap(update(requestErrors, 1)))
