//! Port of `internal/rules/manual_metric_tap_to_track_test.go`.

use ets_bundledeffect::EffectVersion;
use regex::Regex;

use crate::*;

// go test: internal/rules/manual_metric_tap_to_track_test.go:13 TestManualMetricTapToTrackCallbackShapes 160a8a2ce441d10c
#[test]
fn test_manual_metric_tap_to_track_callback_shapes() {
    run_on_checker_stack(|t| {
        t.parallel();
        let source = r#"import { Effect, Metric } from "effect"
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
"#;
        let want = ["verbatim", "ambientMetric", "annotatedReturn"];
        let export_name = Regex::new(r"export const ([0-9A-Za-z_]+)").expect("valid regexp");

        t.run(EffectVersion::V4.as_str(), |t| {
            t.parallel();
            let preview = evaluate_preview(
                t,
                EffectVersion::V4,
                source,
                &ets_rules::MANUAL_METRIC_TAP_TO_TRACK,
            );
            let mut got: Vec<String> = Vec::new();
            for diagnostic in &preview.diagnostics {
                if !diagnostic.text.contains(
                    "`Effect.trackSuccesses` expresses recording a metric on every success directly.",
                ) {
                    t.fatal(format!("unexpected diagnostic text {:?}", diagnostic.text));
                }
                let start = diagnostic.start as usize;
                let line_start =
                    preview.source_text[..start].rfind('\n').map_or(0, |index| index + 1);
                let Some(m) = export_name.captures(&preview.source_text[line_start..start]) else {
                    t.fatal(format!("diagnostic at {} is not on an export line", diagnostic.start));
                };
                got.push(m[1].to_string());
            }
            if got != want {
                t.fatal(format!("matched {got:?}, want {want:?}"));
            }
        });

        // Effect v3 has no Effect.track* combinators.
        t.run(EffectVersion::V3.as_str(), |t| {
            t.parallel();
            let preview = evaluate_preview(
                t,
                EffectVersion::V3,
                source,
                &ets_rules::MANUAL_METRIC_TAP_TO_TRACK,
            );
            if !preview.diagnostics.is_empty() {
                t.fatal(format!(
                    "expected no diagnostics on Effect v3, got {}",
                    preview.diagnostics.len()
                ));
            }
        });
    });
}
