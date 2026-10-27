package rules_test

import (
	"regexp"
	"slices"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/rules"
)

func TestManualMetricTapToTrackCallbackShapes(t *testing.T) {
	t.Parallel()
	source := `import { Effect, Metric } from "effect"
declare const source: Effect.Effect<number>
declare const ambientCounter: Metric.Metric<number, Metric.CounterState<number>>
const counter = Metric.counter("counter")
export const verbatim = source.pipe(Effect.tap((n) => Metric.update(counter, n)))
export const ambientMetric = source.pipe(Effect.tap(() => Metric.update(ambientCounter, 1)))
export const annotatedReturn = source.pipe(Effect.tap((n): Effect.Effect<void> => Metric.update(counter, n)))
export const thisParameter = source.pipe(Effect.tap(function(this: unknown, n: number) { return Metric.update(counter, n) }))
export const typeArguments = source.pipe(Effect.tap<number, void, never, never>((n) => Metric.update(counter, n)))
export const optionalCall = source.pipe(Effect.tap((n) => Metric.update?.(counter, n)))
export const spreadArguments = source.pipe(Effect.tap(() => Metric.update(...([counter, 1] as const))))
export const typedUpdate = source.pipe(Effect.tap((n) => Metric.update<number, Metric.CounterState<number>>(counter, n)))
export const functionArguments = source.pipe(Effect.tap(function() { return Metric.update(counter, arguments.length) }))
export const functionThis = source.pipe(Effect.tap(function(this: void) { return Metric.update(counter, 1) }))
export const asyncCallback = source.pipe(Effect.tap(async () => Metric.update(counter, 1)))
export const genericCallback = source.pipe(Effect.tap(<A>(n: A) => Metric.update(counter, 1)))
`
	want := []string{"verbatim", "ambientMetric", "annotatedReturn"}
	exportName := regexp.MustCompile(`export const (\w+)`)

	t.Run(string(bundledeffect.EffectV4), func(t *testing.T) {
		t.Parallel()
		preview := evaluatePreview(t, bundledeffect.EffectV4, source, &rules.ManualMetricTapToTrack)
		var got []string
		for _, diagnostic := range preview.Diagnostics {
			if !strings.Contains(diagnostic.Text, "`Effect.trackSuccesses` expresses recording a metric on every success directly.") {
				t.Fatalf("unexpected diagnostic text %q", diagnostic.Text)
			}
			lineStart := strings.LastIndex(preview.SourceText[:diagnostic.Start], "\n") + 1
			match := exportName.FindStringSubmatch(preview.SourceText[lineStart:diagnostic.Start])
			if match == nil {
				t.Fatalf("diagnostic at %d is not on an export line", diagnostic.Start)
			}
			got = append(got, match[1])
		}
		if !slices.Equal(got, want) {
			t.Fatalf("matched %v, want %v", got, want)
		}
	})

	// Effect v3 has no Effect.track* combinators.
	t.Run(string(bundledeffect.EffectV3), func(t *testing.T) {
		t.Parallel()
		preview := evaluatePreview(t, bundledeffect.EffectV3, source, &rules.ManualMetricTapToTrack)
		if len(preview.Diagnostics) != 0 {
			t.Fatalf("expected no diagnostics on Effect v3, got %d", len(preview.Diagnostics))
		}
	})
}
