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
}

// go test: internal/effecttest/map_const_to_as_test.go:16 TestMapConstToAsFixesCompile
fn test_map_const_to_as_fixes_compile(t: &T) {
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
      "diagnosticSeverity": { "*": "off", "mapConstToAs": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
"#;
            let cases = [
                Case { name: "mapConstToAs.ts", source: "", expected: "", matches: 23 },
                Case {
                    // A return annotation pins the success type; Effect.as would widen it.
                    name: "annotatedReturn",
                    source: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number>
const result = source.pipe(Effect.map((): "saved" => "saved"))
export const checked: Effect.Effect<"saved"> = result
"#,
                    expected: "",
                    matches: 0,
                },
                Case {
                    name: "dataFirst",
                    source: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = Effect.map(source, () => "saved")
"#,
                    expected: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = Effect.as(source, "saved")
"#,
                    matches: 1,
                },
                Case {
                    name: "dataLastApplied",
                    source: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = Effect.map(() => -1)(source)
"#,
                    expected: r#"import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = Effect.as(-1)(source)
"#,
                    matches: 1,
                },
                Case {
                    name: "aliasFallback",
                    source: r#"import * as Fx from "effect/Effect"
declare const source: Fx.Effect<number>
const mapAlias = Fx.map
export const value = source.pipe(mapAlias(() => "saved"))
"#,
                    expected: r#"import * as Fx from "effect/Effect"
declare const source: Fx.Effect<number>
const mapAlias = Fx.map
export const value = source.pipe(Fx.as("saved"))
"#,
                    matches: 1,
                },
                Case {
                    name: "blockReturnInPipe",
                    source: r#"import { Effect } from "effect"
enum Phase { Done = "done" }
declare const source: Effect.Effect<number>
export const value = source.pipe(
  Effect.tap(() => Effect.void),
  Effect.map((_) => {
    return Phase.Done
  }),
  Effect.map((phase) => phase.length)
)
"#,
                    expected: r#"import { Effect } from "effect"
enum Phase { Done = "done" }
declare const source: Effect.Effect<number>
export const value = source.pipe(
  Effect.tap(() => Effect.void),
  Effect.as(Phase.Done),
  Effect.map((phase) => phase.length)
)
"#,
                    matches: 1,
                },
            ];
            let code = tsdiag::EFFECT_AS_EXPRESSES_REPLACING_THE_SUCCESS_VALUE_WITH_A_CONSTANT_DIRECTLY_EFFECT_MAP_CONST_TO_AS.code();
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
                    let (mut matches, mut fixes) = (0, 0);
                    for diagnostic in &f.get_quick_fixes_for_diagnostics(t, &uri) {
                        if !has_diagnostic_code(&diagnostic.diagnostic, code) {
                            continue;
                        }
                        matches += 1;
                        for (index, title) in diagnostic.fix_titles.iter().enumerate() {
                            if title != "Replace with Effect.as" {
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
                    if matches != test.matches || fixes != test.matches {
                        t.fatal(format!(
                            "got {matches} diagnostics and {fixes} compiled fixes, want {} of each",
                            test.matches
                        ));
                    }
                    f.done(t);
                });
            }
        });
    }
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case("TestMapConstToAsFixesCompile".to_string(), test_map_const_to_as_fixes_compile)]
}
