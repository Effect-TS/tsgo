// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics manualMetricTapToTrack:warning
// Effect v3 has no Effect.trackSuccesses / trackErrors / trackDefects, so the rule never fires here.
import type { Effect } from "effect"
import { tap, tapError } from "effect/Effect"
import { counter, gauge, update } from "effect/Metric"

const responseSize = gauge("response_size")
const requestErrors = counter("request_errors")

declare const handleRequest: Effect.Effect<number, Error>

// MUST NOT TRIGGER (v3 has no Effect.track*): named tap import with a type-only `Effect` import
export const namedTap = handleRequest.pipe(tap((size) => update(responseSize, size)))

// MUST NOT TRIGGER (v3 has no Effect.track*): named tapError import, data-first
export const namedTapError = tapError(handleRequest, () => update(requestErrors, 1))

// MUST NOT TRIGGER (v3 has no Effect.track*): named tap and update imports, direct effect
export const namedTapDirect = handleRequest.pipe(tap(update(requestErrors, 1)))
