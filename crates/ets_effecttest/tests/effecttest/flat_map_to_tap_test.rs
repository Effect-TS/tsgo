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

// go test: internal/effecttest/flat_map_to_tap_test.go:17 TestFlatMapToTapFixesCompile
fn test_flat_map_to_tap_fixes_compile(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    for version in [EffectVersion::V3, EffectVersion::V4] {
        t.run(version.as_str(), |t| {
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
      "diagnosticSeverity": { "*": "off", "flatMapToTap": "suggestion", "promiseInEffectSuccess": "off" }
    }]
  }
}
// @filename: /repro.ts
"#;
            t.parallel();
            let mut main_matches = 23;
            if version == EffectVersion::V3 {
                main_matches = 22;
            }
            let cases = [
                Case {
                    name: "flatMapToTap.ts",
                    source: "",
                    expected: "",
                    matches: main_matches,
                    fixes: main_matches,
                },
                Case {
                    name: "flatMapToTap_aliases.ts",
                    source: "",
                    expected: "",
                    matches: 6,
                    fixes: 6,
                },
                Case {
                    name: "flatMapToTap_preview.ts",
                    source: "",
                    expected: "",
                    matches: 1,
                    fixes: 1,
                },
                Case {
                    name: "flatMapToTap_noTarget.ts",
                    source: "",
                    expected: "",
                    matches: 1,
                    fixes: 0,
                },
                Case {
                    name: "exactNamedTap",
                    source: r#"import type { Effect } from "effect"
import { as as restore, flatMap as bind, tap as observe } from "effect/Effect"
declare const source: Effect.Effect<number>
declare const audit: Effect.Effect<void>
const aTap: unknown = observe
export const value = source.pipe(bind((n) => restore(audit, n)))
void aTap
"#,
                    expected: r#"import type { Effect } from "effect"
import { as as restore, flatMap as bind, tap as observe } from "effect/Effect"
declare const source: Effect.Effect<number>
declare const audit: Effect.Effect<void>
const aTap: unknown = observe
export const value = source.pipe(observe((n) => audit))
void aTap
"#,
                    matches: 1,
                    fixes: 1,
                },
                Case {
                    name: "exactDirect",
                    source: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = Effect.flatMap(source, (n) => Effect.as(Effect.log(n), n))
"#,
                    expected: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = Effect.tap(source, (n) => Effect.log(n))
"#,
                    matches: 1,
                    fixes: 1,
                },
                Case {
                    name: "exactPipeComment",
                    source: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.flatMap((n) => Effect.log(n).pipe(
  Effect.map(() => 1), // Keep this explanation of the prior step.
  Effect.as(n)
)))
"#,
                    expected: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.tap((n) => Effect.log(n).pipe(
  Effect.map(() => 1) // Keep this explanation of the prior step.

)))
"#,
                    matches: 1,
                    fixes: 1,
                },
                Case {
                    name: "exactPipe",
                    source: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.andThen((n) => {
  // Keep the callback's logging.
  console.log(n)
  return Effect.log(/* keep this argument */ n).pipe(Effect.map(() => 1), Effect.as(n))
}))
"#,
                    expected: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.tap((n) => {
  // Keep the callback's logging.
  console.log(n)
  return Effect.log(/* keep this argument */ n).pipe(Effect.map(() => 1))
}))
"#,
                    matches: 1,
                    fixes: 1,
                },
            ];
            let code = tsdiag::EFFECT_TAP_EXPRESSES_THIS_OBSERVATION_MORE_DIRECTLY_THAN_EFFECT_0_WITH_A_MANUAL_RE_YIELD_EFFECT_FLAT_MAP_TO_TAP.code();
            let uri = lsconv::file_name_to_document_uri("/repro.ts");
            for test in &cases {
                t.run(test.name, |t| {
                    let mut source = test.source.to_string();
                    if source.is_empty() {
                        source = match std::fs::read_to_string(
                            ets_effecttest::test_data_path()
                                .join("tests")
                                .join(version.as_str())
                                .join(test.name),
                        ) {
                            Ok(fixture) => fixture,
                            Err(err) => t.fatal(err.to_string()),
                        };
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
                        let before = checked;
                        for (index, title) in diagnostic.fix_titles.iter().enumerate() {
                            if title != "Replace with Effect.tap" {
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
                            if test.name == "flatMapToTap.ts"
                                && result.changes[0].before.contains("original user")
                                && !after.contains("original user")
                            {
                                t.fatal("replacement removed an inner comment");
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
                                for diagnostic in converted.get_quick_fixes_for_diagnostics(t, &uri)
                                {
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
                        if before == checked {
                            t.log(format!(
                                "no fix at line {}",
                                diagnostic.diagnostic.range.start.line + 1
                            ));
                        }
                    }
                    if matches != test.matches || checked != test.fixes {
                        t.fatal(format!(
                            "got {matches} diagnostics and {checked} compiled fixes, want {} and {}",
                            test.matches, test.fixes
                        ));
                    }
                    t.log(format!("compiled {checked} tap replacements"));
                    f.done(t);
                });
            }
        });
    }
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case("TestFlatMapToTapFixesCompile".to_string(), test_flat_map_to_tap_fixes_compile)]
}
