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

// go test: internal/effecttest/flat_map_identity_to_flatten_test.go:17 TestFlatMapIdentityToFlattenFixesCompile
fn test_flat_map_identity_to_flatten_fixes_compile(t: &T) {
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
      "diagnosticSeverity": { "*": "off", "flatMapIdentityToFlatten": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
"#;
            let mut main_matches = 18;
            if version == EffectVersion::V3 {
                main_matches = 19;
            }
            let mut cases = vec![
                Case {
                    name: "flatMapIdentityToFlatten.ts",
                    source: "",
                    expected: "",
                    matches: main_matches,
                    fixes: main_matches,
                },
                Case {
                    name: "exactPipeable",
                    source: r#"import * as Fx from "effect/Effect"
declare const source: Fx.Effect<Fx.Effect<number>>
export const value = source.pipe(Fx.flatMap((inner) => /* unwrap */ inner))
"#,
                    expected: r#"import * as Fx from "effect/Effect"
declare const source: Fx.Effect<Fx.Effect<number>>
export const value = source.pipe(Fx.flatten)
"#,
                    matches: 1,
                    fixes: 1,
                },
                Case {
                    name: "exactDataFirst",
                    source: r#"import { Effect, identity } from "effect"
declare const source: Effect.Effect<Effect.Effect<number>>
export const value = Effect.flatMap(source, identity)
"#,
                    expected: r#"import { Effect, identity } from "effect"
declare const source: Effect.Effect<Effect.Effect<number>>
export const value = Effect.flatten(source)
"#,
                    matches: 1,
                    fixes: 1,
                },
                Case {
                    name: "dataLastIgnored",
                    source: r#"import { Effect } from "effect"
declare const source: Effect.Effect<Effect.Effect<number>>
export const value = Effect.flatMap((inner: Effect.Effect<number>) => inner)(source)
"#,
                    expected: "",
                    matches: 0,
                    fixes: 0,
                },
                Case {
                    name: "typedAliasWideningError",
                    source: r#"import { Effect, identity } from "effect"
declare const source: Effect.Effect<Effect.Effect<string>>
const unwrap: (x: Effect.Effect<string>) => Effect.Effect<string, Error> = identity
export const value = source.pipe(Effect.flatMap(unwrap))
export const handled = value.pipe(Effect.mapError((error) => error.message))
"#,
                    expected: "",
                    matches: 0,
                    fixes: 0,
                },
                Case {
                    name: "typedAliasAnySuccess",
                    source: r#"import { Effect, identity } from "effect"
declare const source: Effect.Effect<Effect.Effect<string>>
const unwrap: (x: Effect.Effect<string>) => Effect.Effect<any> = identity
export const value = source.pipe(Effect.flatMap(unwrap))
export const mapped = value.pipe(Effect.map((value) => value.toFixed()))
"#,
                    expected: "",
                    matches: 0,
                    fixes: 0,
                },
                Case {
                    name: "typedAliasMatchingIgnored",
                    source: r#"import { Effect, identity } from "effect"
declare const source: Effect.Effect<Effect.Effect<string>>
const unwrap: (x: Effect.Effect<string>) => Effect.Effect<string> = identity
export const value = source.pipe(Effect.flatMap(unwrap))
"#,
                    expected: "",
                    matches: 0,
                    fixes: 0,
                },
                Case {
                    name: "namedFlatMapHasNoFix",
                    source: r#"import type { Effect } from "effect"
import { flatMap } from "effect/Effect"
declare const source: Effect.Effect<Effect.Effect<number>>
export const value = source.pipe(flatMap((inner) => inner))
"#,
                    expected: "",
                    matches: 1,
                    fixes: 0,
                },
            ];
            if version == EffectVersion::V4 {
                cases.push(Case {
                    name: "flatMapIdentityToFlatten_preview.ts",
                    source: "",
                    expected: "",
                    matches: 1,
                    fixes: 1,
                });
            }
            let code = tsdiag::EFFECT_FLATTEN_EXPRESSES_THIS_UNWRAPPING_MORE_DIRECTLY_THAN_EFFECT_FLAT_MAP_WITH_AN_IDENTITY_CALLBACK_EFFECT_FLAT_MAP_IDENTITY_TO_FLATTEN.code();
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
                        // The fixtures raise the rule to a warning; keep it a suggestion so that
                        // only real compile errors fail VerifyNonSuggestionDiagnostics.
                        source = fixture.replacen(
                            "flatMapIdentityToFlatten:warning",
                            "flatMapIdentityToFlatten:suggestion",
                            1,
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
                            if title != "Replace with Effect.flatten" {
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
        "TestFlatMapIdentityToFlattenFixesCompile".to_string(),
        test_flat_map_identity_to_flatten_fixes_compile,
    )]
}
