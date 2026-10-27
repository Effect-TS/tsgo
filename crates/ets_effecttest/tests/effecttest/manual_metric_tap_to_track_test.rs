// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::diagnostics as tsdiag;
use ets_shim::ls::lsconv;
use libtest_mimic::Trial;

use crate::{has_diagnostic_code, run_case};

/// Rust-only: one entry of the Go test's anonymous `cases` struct slice.
struct Case {
    name: &'static str,
    source: &'static str,
    expected: &'static str,
    matches: i32,
    fixes: i32,
}

// go test: internal/effecttest/manual_metric_tap_to_track_test.go:17 TestManualMetricTapToTrackFixesCompile c91646929b055dbe
fn test_manual_metric_tap_to_track_fixes_compile(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    let version = EffectVersion::V4;
    const PREFIX: &str = r#"// @filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "skipLibCheck": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [{
      "name": "@effect/language-service",
      "diagnosticSeverity": { "*": "off", "manualMetricTapToTrack": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
"#;
    let fixture = |name, matches, fixes| Case { name, source: "", expected: "", matches, fixes };
    let cases = [
        fixture("manualMetricTapToTrack.ts", 30, 30),
        fixture("manualMetricTapToTrack_preview.ts", 1, 1),
        fixture("manualMetricTapToTrack_noTarget.ts", 3, 0),
        fixture("manualMetricTapToTrack_bindings.ts", 7, 6),
        fixture("manualMetricTapToTrack_localAliases.ts", 3, 2),
        Case {
            name: "dataFirstCallback",
            source: r#"import { Effect, Metric } from "effect"
const responseSize = Metric.gauge("response_size")
declare const source: Effect.Effect<number>
export const value = Effect.tap(source, (size) => Metric.update(responseSize, size))
"#,
            expected: r#"import { Effect, Metric } from "effect"
const responseSize = Metric.gauge("response_size")
declare const source: Effect.Effect<number>
export const value = Effect.trackSuccesses(source, responseSize)
"#,
            matches: 1,
            fixes: 1,
        },
        Case {
            name: "dataFirstDirect",
            source: r#"import { Effect, Metric } from "effect"
const successes = Metric.counter("successes")
declare const source: Effect.Effect<number>
export const value = Effect.tap(source, Metric.update(successes, 1))
"#,
            expected: r#"import { Effect, Metric } from "effect"
const successes = Metric.counter("successes")
declare const source: Effect.Effect<number>
export const value = Effect.trackSuccesses(source, Metric.withConstantInput(successes, 1))
"#,
            matches: 1,
            fixes: 1,
        },
        Case {
            name: "directNamespaceMetric",
            source: r#"import { Effect } from "effect"
import * as M from "effect/Metric"
const successes = M.counter("successes")
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.tap(M.update(successes, 1)))
"#,
            expected: r#"import { Effect } from "effect"
import * as M from "effect/Metric"
const successes = M.counter("successes")
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.trackSuccesses(M.withConstantInput(successes, 1)))
"#,
            matches: 1,
            fixes: 1,
        },
        Case {
            name: "directUpdateAliasFallback",
            source: r#"import { Effect, Metric } from "effect"
const successes = Metric.counter("successes")
const updateAlias = Metric.update
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.tap(updateAlias(successes, 1)))
"#,
            expected: r#"import { Effect, Metric } from "effect"
const successes = Metric.counter("successes")
const updateAlias = Metric.update
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.trackSuccesses(Metric.withConstantInput(successes, 1)))
"#,
            matches: 1,
            fixes: 1,
        },
        Case {
            name: "tapAliasFallback",
            source: r#"import * as Fx from "effect/Effect"
import { Metric } from "effect"
const responseSize = Metric.gauge("response_size")
const tapAlias = Fx.tap
declare const source: Fx.Effect<number>
export const value = source.pipe(tapAlias((size) => Metric.update(responseSize, size)))
"#,
            expected: r#"import * as Fx from "effect/Effect"
import { Metric } from "effect"
const responseSize = Metric.gauge("response_size")
const tapAlias = Fx.tap
declare const source: Fx.Effect<number>
export const value = source.pipe(Fx.trackSuccesses(responseSize))
"#,
            matches: 1,
            fixes: 1,
        },
        Case {
            name: "destructuredMapper",
            source: r#"import { Effect, Metric } from "effect"
const userCount = Metric.gauge("user_count")
declare const source: Effect.Effect<ReadonlyArray<string>>
export const value = source.pipe(Effect.tap(({ length }) => Metric.update(userCount, length)))
"#,
            expected: r#"import { Effect, Metric } from "effect"
const userCount = Metric.gauge("user_count")
declare const source: Effect.Effect<ReadonlyArray<string>>
export const value = source.pipe(Effect.trackSuccesses(userCount, ({ length }) => length))
"#,
            matches: 1,
            fixes: 1,
        },
        Case {
            name: "restParameterMapper",
            source: r#"import { Effect, Metric } from "effect"
const calls = Metric.gauge("calls")
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.tap((...args: Array<number>) => Metric.update(calls, args.length)))
"#,
            expected: r#"import { Effect, Metric } from "effect"
const calls = Metric.gauge("calls")
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.trackSuccesses(calls, (...args: Array<number>) => args.length))
"#,
            matches: 1,
            fixes: 1,
        },
        Case {
            name: "defaultParameterMapper",
            source: r#"import { Effect, Metric } from "effect"
const sizes = Metric.gauge("sizes")
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.tap((size = 0) => Metric.update(sizes, size)))
"#,
            expected: r#"import { Effect, Metric } from "effect"
const sizes = Metric.gauge("sizes")
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.trackSuccesses(sizes, (size = 0) => size))
"#,
            matches: 1,
            fixes: 1,
        },
        Case {
            name: "effectFnPipeable",
            source: r#"import { Effect, Metric } from "effect"
const responseSize = Metric.gauge("response_size")
export const handler = Effect.fn("handler")(
  function*(id: string) {
    return id.length
  },
  Effect.tap((size) => Metric.update(responseSize, size))
)
"#,
            expected: r#"import { Effect, Metric } from "effect"
const responseSize = Metric.gauge("response_size")
export const handler = Effect.fn("handler")(
  function*(id: string) {
    return id.length
  },
  Effect.trackSuccesses(responseSize)
)
"#,
            matches: 1,
            fixes: 1,
        },
        Case {
            name: "dataLastApplied",
            source: r#"import { Effect, Metric } from "effect"
const requestErrors = Metric.counter("request_errors")
declare const source: Effect.Effect<number, Error>
export const value = Effect.tapError(() => Metric.update(requestErrors, 1))(source)
"#,
            expected: r#"import { Effect, Metric } from "effect"
const requestErrors = Metric.counter("request_errors")
declare const source: Effect.Effect<number, Error>
export const value = Effect.trackErrors(requestErrors, () => 1)(source)
"#,
            matches: 1,
            fixes: 1,
        },
        Case {
            // Only direct member imports reach the target APIs.
            name: "directMemberImports",
            source: r#"import type { Effect } from "effect"
import { tap, trackSuccesses } from "effect/Effect"
import { counter, update, withConstantInput } from "effect/Metric"
const successes = counter("successes")
declare const source: Effect.Effect<number>
export const value = source.pipe(tap(update(successes, 1)))
"#,
            expected: r#"import type { Effect } from "effect"
import { tap, trackSuccesses } from "effect/Effect"
import { counter, update, withConstantInput } from "effect/Metric"
const successes = counter("successes")
declare const source: Effect.Effect<number>
export const value = source.pipe(trackSuccesses(withConstantInput(successes, 1)))
"#,
            matches: 1,
            fixes: 1,
        },
        Case {
            // Effect.trackSuccesses is reachable but Metric.withConstantInput is not.
            name: "noMetricTarget",
            source: r#"import { Effect } from "effect"
import { counter, update } from "effect/Metric"
const successes = counter("successes")
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.tap(update(successes, 1)))
"#,
            expected: "",
            matches: 1,
            fixes: 0,
        },
    ];
    let code = tsdiag::EFFECT_0_EXPRESSES_RECORDING_A_METRIC_ON_EVERY_1_DIRECTLY_EFFECT_MANUAL_METRIC_TAP_TO_TRACK.code();
    let uri = lsconv::file_name_to_document_uri("/repro.ts");
    let count_matches = |f: &mut ets_fourslash::FourslashTest| -> i32 {
        let mut count = 0;
        for diagnostic in f.get_quick_fixes_for_diagnostics(t, &uri) {
            if has_diagnostic_code(&diagnostic.diagnostic, code) {
                count += 1;
            }
        }
        count
    };
    for test in &cases {
        t.run(test.name, |t| {
            t.parallel();
            let mut source = test.source.to_string();
            if source.is_empty() {
                let fixture = match std::fs::read_to_string(
                    ets_effecttest::test_data_path()
                        .join("tests")
                        .join(version.as_str())
                        .join(test.name),
                ) {
                    Ok(fixture) => fixture,
                    Err(err) => t.fatal(err.to_string()),
                };
                // The fixtures raise the rule to a warning; keep it a suggestion so that
                // only real compile errors fail VerifyNonSuggestionDiagnostics.
                source = fixture.replacen(
                    "manualMetricTapToTrack:warning",
                    "manualMetricTapToTrack:suggestion",
                    1,
                );
            }
            let source = format!("// @{}\n{source}", version.as_str());
            let mut f = ets_fourslash::new_fourslash(t, None, &format!("{PREFIX}{source}"));
            f.go_to_file(t, "/repro.ts");
            f.verify_non_suggestion_diagnostics(t, &[]);
            let (mut matches, mut fixes) = (0, 0);
            for diagnostic in &f.get_quick_fixes_for_diagnostics(t, &uri) {
                if !has_diagnostic_code(&diagnostic.diagnostic, code) {
                    continue;
                }
                matches += 1;
                for (index, title) in diagnostic.fix_titles.iter().enumerate() {
                    if !title.starts_with("Replace with Effect.track") {
                        continue;
                    }
                    let result = f.apply_quick_fix(t, &uri, &diagnostic.diagnostic, index);
                    if result.changes.len() != 1 {
                        t.fatal(format!("expected one changed file, got {}", result.changes.len()));
                    }
                    let after = &result.changes[0].after;
                    let want = format!("// @{}\n{}", version.as_str(), test.expected);
                    if !test.expected.is_empty() && *after != want {
                        t.fatal(format!("replacement = {after:?}, want {want:?}"));
                    }
                    {
                        let mut converted =
                            ets_fourslash::new_fourslash(t, None, &format!("{PREFIX}{after}"));
                        converted.go_to_file(t, "/repro.ts");
                        converted.verify_non_suggestion_diagnostics(t, &[]);
                        let remaining = count_matches(&mut converted);
                        if remaining != test.matches - 1 {
                            t.fatal(format!(
                                "expected {} diagnostics after replacement, got {remaining}",
                                test.matches - 1
                            ));
                        }
                        converted.done(t);
                    }
                    fixes += 1;
                }
            }
            if matches != test.matches || fixes != test.fixes {
                t.fatal(format!(
                    "got {matches} diagnostics and {fixes} compiled fixes, want {} and {}",
                    test.matches, test.fixes
                ));
            }
            f.done(t);
        });
    }
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case(
        "TestManualMetricTapToTrackFixesCompile".to_string(),
        test_manual_metric_tap_to_track_fixes_compile,
    )]
}
