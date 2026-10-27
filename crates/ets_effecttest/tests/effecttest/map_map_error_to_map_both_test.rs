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

/// Rust-only: a fixture case of the Go test (`{name: ..., matches: ..., fixes: ...}`).
const fn fixture(name: &'static str, matches: i32, fixes: i32) -> Case {
    Case { name, source: "", expected: "", matches, fixes }
}

// go test: internal/effecttest/map_map_error_to_map_both_test.go:17 TestMapMapErrorToMapBothFixesCompile 0d0eae3fead7a055
fn test_map_map_error_to_map_both_fixes_compile(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    for version in [EffectVersion::V3, EffectVersion::V4] {
        t.run(version.as_str(), |t| {
            t.parallel();
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
      "diagnosticSeverity": { "*": "off", "mapMapErrorToMapBoth": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
"#;
            let (mut main_matches, mut main_fixes) = (25, 24);
            // Eager pairs split across an evaluation point have no fix unless the
            // second callback is a function literal.
            let (mut order_matches, mut order_fixes) = (6, 3);
            let (mut cross_matches, mut cross_fixes) = (11, 9);
            // Curried pairs keep a fix only when no non-literal expression is reordered
            // and no call they edit has explicit type arguments.
            let (mut curried_matches, mut curried_fixes) = (10, 3);
            // Pairs split across calls keep a fix only when no non-literal callback or
            // eager work moves across other code, and no call on the path between the
            // steps has explicit type arguments.
            let (mut boundary_matches, mut boundary_fixes) = (9, 4);
            if version == EffectVersion::V3 {
                (curried_matches, curried_fixes) = (8, 2);
                (boundary_matches, boundary_fixes) = (6, 3);
                (main_matches, main_fixes) = (22, 21);
                (order_matches, order_fixes) = (2, 2);
                (cross_matches, cross_fixes) = (8, 8);
            }
            let cases = [
                fixture("mapMapErrorToMapBoth.ts", main_matches, main_fixes),
                fixture("mapMapErrorToMapBoth_preview.ts", 1, 1),
                // No binding in scope reaches mapBoth, so no fix is offered.
                fixture("mapMapErrorToMapBoth_namedImportsNoTarget.ts", 3, 0),
                fixture("mapMapErrorToMapBoth_namedImportsTarget.ts", 1, 1),
                fixture("mapMapErrorToMapBoth_evaluationOrder.ts", order_matches, order_fixes),
                fixture("mapMapErrorToMapBoth_crossPipe.ts", cross_matches, cross_fixes),
                fixture("mapMapErrorToMapBoth_curried.ts", curried_matches, curried_fixes),
                fixture("mapMapErrorToMapBoth_boundaries.ts", boundary_matches, boundary_fixes),
                // Computed callees have no fix: removing or replacing them would drop evaluation.
                fixture("mapMapErrorToMapBoth_calleeExpressions.ts", 4, 1),
                // A low-precedence subject that becomes a receiver must be parenthesized.
                fixture("mapMapErrorToMapBoth_precedence.ts", 8, 8),
                // No fix may newly start a statement with `(`, which joins it onto the previous line.
                fixture("mapMapErrorToMapBoth_asi.ts", 6, 3),
                Case {
                    name: "exactCrossPipe",
                    source: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const value = source.pipe(Effect.tap(() => Effect.void), Effect.map((n) => n + 1)).pipe(Effect.mapError((e) => e.length), Effect.asVoid)
"#,
                    expected: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const value = source.pipe(Effect.tap(() => Effect.void)).pipe(Effect.mapBoth({
    onSuccess: n => n + 1,
    onFailure: e => e.length
}), Effect.asVoid)
"#,
                    matches: 1,
                    fixes: 1,
                },
                Case {
                    name: "exactEmptiedPipe",
                    source: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const value = source.pipe(Effect.mapError((e) => e.length)).pipe(Effect.map((n) => n + 1))
"#,
                    expected: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const value = source.pipe(Effect.mapBoth({
    onFailure: e => e.length,
    onSuccess: n => n + 1
}))
"#,
                    matches: 1,
                    fixes: 1,
                },
                Case {
                    name: "exactNamedImportTarget",
                    source: r#"import type { Effect } from "effect"
import { map, mapBoth, mapError } from "effect/Effect"
declare const source: Effect.Effect<number, string>
export const value = source.pipe(map((n) => n + 1), mapError((e) => e.length))
"#,
                    expected: r#"import type { Effect } from "effect"
import { map, mapBoth, mapError } from "effect/Effect"
declare const source: Effect.Effect<number, string>
export const value = source.pipe(mapBoth({
    onSuccess: n => n + 1,
    onFailure: e => e.length
}))
"#,
                    matches: 1,
                    fixes: 1,
                },
                Case {
                    name: "exactReverseOrder",
                    source: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const value = source.pipe(Effect.mapError((e) => e.length), Effect.map((n) => n + 1))
"#,
                    expected: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const value = source.pipe(Effect.mapBoth({
    onFailure: e => e.length,
    onSuccess: n => n + 1
}))
"#,
                    matches: 1,
                    fixes: 1,
                },
                Case {
                    name: "exactNamespaceAndSurroundingSteps",
                    source: r#"import * as E from "effect/Effect"
declare const source: E.Effect<number, string>
export const value = source.pipe(E.tap(() => E.void), E.map((n) => n + 1), E.mapError((e) => e.length), E.asVoid)
"#,
                    expected: r#"import * as E from "effect/Effect"
declare const source: E.Effect<number, string>
export const value = source.pipe(E.tap(() => E.void), E.mapBoth({
    onSuccess: n => n + 1,
    onFailure: e => e.length
}), E.asVoid)
"#,
                    matches: 1,
                    fixes: 1,
                },
                Case {
                    name: "exactDataFirst",
                    source: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const value = Effect.map(Effect.mapError(source, (e) => e.length), (n) => n + 1)
"#,
                    expected: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const value = Effect.mapBoth(source, {
    onFailure: e => e.length,
    onSuccess: n => n + 1
})
"#,
                    matches: 1,
                    fixes: 1,
                },
            ];
            let code = tsdiag::EFFECT_0_EFFECT_1_TRANSFORM_THE_SUCCESS_AND_FAILURE_CHANNELS_ONE_STEP_AT_A_TIME_EFFECT_2_EXPRESSES_BOTH_TRANSFORMATIONS_IN_A_SINGLE_STEP_EFFECT_MAP_MAP_ERROR_TO_MAP_BOTH.code();
            let uri = lsconv::file_name_to_document_uri("/repro.ts");
            for test in &cases {
                t.run(test.name, |t| {
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
                        // Keep the rule at suggestion severity so only compile errors are non-suggestions.
                        source = fixture.replace(
                            "mapMapErrorToMapBoth:warning",
                            "mapMapErrorToMapBoth:suggestion",
                        );
                    }
                    let source = format!("// @{}\n{source}", version.as_str());
                    let mut f = ets_fourslash::new_fourslash(t, None, &format!("{PREFIX}{source}"));
                    f.go_to_file(t, "/repro.ts");
                    f.verify_non_suggestion_diagnostics(t, &[]);
                    let inventory = f.get_quick_fixes_for_diagnostics(t, &uri);
                    let (mut matches, mut checked) = (0, 0);
                    for diagnostic in &inventory {
                        if !has_diagnostic_code(&diagnostic.diagnostic, code) {
                            continue;
                        }
                        matches += 1;
                        for (index, title) in diagnostic.fix_titles.iter().enumerate() {
                            if title != "Replace with Effect.mapBoth"
                                && title != "Replace with Effect.mapBothEager"
                            {
                                continue;
                            }
                            let result = f.apply_quick_fix(t, &uri, &diagnostic.diagnostic, index);
                            if result.changes.len() != 1 {
                                t.fatal(format!(
                                    "expected one changed file, got {}",
                                    result.changes.len()
                                ));
                            }
                            let after = &result.changes[0].after;
                            let want = format!("// @{}\n{}", version.as_str(), test.expected);
                            if !test.expected.is_empty() && *after != want {
                                t.fatal(format!("replacement = {after:?}, want {want:?}"));
                            }
                            {
                                let mut converted = ets_fourslash::new_fourslash(
                                    t,
                                    None,
                                    &format!("{PREFIX}{after}"),
                                );
                                converted.go_to_file(t, "/repro.ts");
                                converted.verify_non_suggestion_diagnostics(t, &[]);
                                let mut remaining = 0;
                                for diagnostic in converted.get_quick_fixes_for_diagnostics(t, &uri) {
                                    if has_diagnostic_code(&diagnostic.diagnostic, code) {
                                        remaining += 1;
                                    }
                                }
                                if remaining != test.matches - 1 {
                                    t.fatal(format!(
                                        "expected {} diagnostics after replacement, got {remaining}",
                                        test.matches - 1
                                    ));
                                }
                                converted.done(t);
                            }
                            checked += 1;
                        }
                    }
                    if matches != test.matches || checked != test.fixes {
                        t.fatal(format!(
                            "got {matches} diagnostics and {checked} compiled fixes, want {} and {}",
                            test.matches, test.fixes
                        ));
                    }
                    f.done(t);
                });
            }
        });
    }
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case(
        "TestMapMapErrorToMapBothFixesCompile".to_string(),
        test_map_map_error_to_map_both_fixes_compile,
    )]
}
