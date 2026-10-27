// @effect-diagnostics *:off
// @effect-diagnostics manualMetricTapToTrack:warning
import { Effect, Metric } from "effect"

const responseSize = Metric.gauge("response_size")

declare const handleRequest: Effect.Effect<number, Error>

export const instrumented = handleRequest.pipe(Effect.tap((size) => Metric.update(responseSize, size)))
