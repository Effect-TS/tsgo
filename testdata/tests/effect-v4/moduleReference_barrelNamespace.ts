// @effect-diagnostics *:off
// @effect-diagnostics effectMapVoid:warning
// @effect-diagnostics flatMapToTap:warning
// @effect-diagnostics mapMapErrorToMapBoth:warning
// @effect-diagnostics manualMetricTapToTrack:warning
// @effect-diagnostics schemaStructWithTag:warning
import * as X from "effect"

declare const source: X.Effect.Effect<number, string>
declare const audit: (n: number) => X.Effect.Effect<string>
const requests = X.Metric.counter("requests")

// Fixes reference the modules through the barrel namespace import: X.Effect.asVoid
export const mapVoid = source.pipe(
  X.Effect.map(() => undefined)
)

// The receiver is a local alias of the module; the fix still uses the import
const E = X.Effect
export const aliasedMapVoid = source.pipe(
  E.map(() => undefined)
)

// X.Effect.tap is resolved through the barrel namespace
export const flatMapToTap = source.pipe(
  X.Effect.flatMap((n) => audit(n).pipe(X.Effect.as(n)))
)

// X.Effect.mapBoth is resolved through the barrel namespace
export const mapMapError = source.pipe(
  X.Effect.map((n) => n > 0),
  X.Effect.mapError((message) => new Error(message))
)

// X.Effect.trackSuccesses and X.Metric.withConstantInput are resolved through the barrel namespace
export const metricTap = source.pipe(
  X.Effect.tap(X.Metric.update(requests, 1))
)

// X.Schema.TaggedStruct
export const Tagged = X.Schema.Struct({
  _tag: X.Schema.Literal("Tagged"),
  value: X.Schema.Number
})
