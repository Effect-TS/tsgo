// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::lsconv;
use libtest_mimic::Trial;

use crate::{has_diagnostic_code, run_case};

/// Rust-only: the rule's message (Go spells it out where it takes the code).
static MESSAGE: &Message = &tsdiag::EFFECT_0_BUILDS_THIS_1_LAZILY_DIRECTLY_INSTEAD_OF_WRAPPING_EFFECT_2_IN_EFFECT_SUSPEND_EFFECT_SUSPEND_FAIL_TO_FAIL_SYNC;

/// Rust-only: Go's function-local `fixCase` type. The sources are built from the version's Effect import, so they
/// are owned.
struct FixCase {
    name: &'static str,
    source: String,
    expected: String,
    matches: i32,
    fixes: i32,
}

// go test: internal/effecttest/suspend_fail_to_fail_sync_test.go:17 TestSuspendFailToFailSyncFixesCompile 0711c169bb7cd162
fn test_suspend_fail_to_fail_sync_fixes_compile(t: &T) {
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
      "diagnosticSeverity": { "*": "off", "suspendFailToFailSync": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
"#;
            let mut effect_import = r#"import { Effect } from "effect""#;
            if version == EffectVersion::V3 {
                effect_import = r#"import * as Effect from "effect/Effect""#;
            }
            // exact fails to compile unless the value keeps exactly the given type,
            // so a fix that changes the inferred error type does not compile.
            const EXACT: &str = r"type Exact<A, B> = (<T>() => T extends A ? 1 : 2) extends (<T>() => T extends B ? 1 : 2) ? true : false
";
            let mut cases = vec![
                FixCase {
                    // A single literal widens to string in both forms.
                    name: "singleLiteral",
                    source: format!(
                        r#"{effect_import}
{EXACT}export const value = Effect.suspend(() => Effect.fail("not found"))
export const check: Exact<typeof value, Effect.Effect<never, string>> = true
"#
                    ),
                    expected: format!(
                        r#"{effect_import}
{EXACT}export const value = Effect.failSync(() => "not found")
export const check: Exact<typeof value, Effect.Effect<never, string>> = true
"#
                    ),
                    matches: 1,
                    fixes: 1,
                },
                FixCase {
                    name: "innerTypeArgument",
                    source: format!(
                        r"{effect_import}
{EXACT}declare const error: Error
export const value = Effect.suspend(() => Effect.fail<Error | string>(error))
export const check: Exact<typeof value, Effect.Effect<never, Error | string>> = true
"
                    ),
                    expected: format!(
                        r"{effect_import}
{EXACT}declare const error: Error
export const value = Effect.failSync<Error | string>(() => error)
export const check: Exact<typeof value, Effect.Effect<never, Error | string>> = true
"
                    ),
                    matches: 1,
                    fixes: 1,
                },
                FixCase {
                    // A bare object literal as a concise arrow body would parse as a block.
                    name: "objectLiteral",
                    source: format!(
                        r#"{effect_import}
{EXACT}export const value = Effect.suspend(() => Effect.fail({{ _tag: "Oops" }}))
export const check: Exact<typeof value, Effect.Effect<never, {{ _tag: string }}>> = true
"#
                    ),
                    expected: format!(
                        r#"{effect_import}
{EXACT}export const value = Effect.failSync(() => ({{ _tag: "Oops" }}))
export const check: Exact<typeof value, Effect.Effect<never, {{ _tag: string }}>> = true
"#
                    ),
                    matches: 1,
                    fixes: 1,
                },
                FixCase {
                    name: "unionBinding",
                    source: format!(
                        r#"{effect_import}
{EXACT}declare const tag: "A" | "B"
export const value = Effect.suspend(() => Effect.fail(tag))
export const check: Exact<typeof value, Effect.Effect<never, "A" | "B">> = true
"#
                    ),
                    expected: format!(
                        r#"{effect_import}
{EXACT}declare const tag: "A" | "B"
export const value = Effect.failSync(() => tag)
export const check: Exact<typeof value, Effect.Effect<never, "A" | "B">> = true
"#
                    ),
                    matches: 1,
                    fixes: 1,
                },
                FixCase {
                    // The suspend and the fail as pipe steps keep the error type.
                    name: "pipeSteps",
                    source: format!(
                        r#"{effect_import}
import {{ pipe }} from "effect/Function"
{EXACT}declare const tag: "A" | "B"
export const value = pipe(() => pipe(tag, Effect.fail), Effect.suspend)
export const check: Exact<typeof value, Effect.Effect<never, "A" | "B">> = true
"#
                    ),
                    expected: format!(
                        r#"{effect_import}
import {{ pipe }} from "effect/Function"
{EXACT}declare const tag: "A" | "B"
export const value = pipe(() => tag, Effect.failSync)
export const check: Exact<typeof value, Effect.Effect<never, "A" | "B">> = true
"#
                    ),
                    matches: 1,
                    fixes: 1,
                },
                FixCase {
                    // Effect.fail widens the literal union to string; a failSync thunk would
                    // keep "A" | "B", so the fix is withheld.
                    name: "literalUnion",
                    source: format!(
                        r#"{effect_import}
declare const condition: boolean
export const value = Effect.suspend(() => Effect.fail(condition ? "A" : "B"))
"#
                    ),
                    expected: String::new(),
                    matches: 1,
                    fixes: 0,
                },
                FixCase {
                    name: "namedImports",
                    source: format!(
                        r#"{effect_import}
import {{ fail, suspend }} from "effect/Effect"
declare const error: Error
export const value = suspend(() => fail(error))
"#
                    ),
                    expected: format!(
                        r#"{effect_import}
import {{ fail, suspend }} from "effect/Effect"
declare const error: Error
export const value = Effect.failSync(() => error)
"#
                    ),
                    matches: 1,
                    fixes: 1,
                },
                FixCase {
                    // Without an Effect module binding the fallback receiver would be unbound.
                    name: "namedImportsOnly",
                    source: r#"import { fail, suspend } from "effect/Effect"
declare const error: Error
export const value = suspend(() => fail(error))
"#
                    .to_string(),
                    expected: String::new(),
                    matches: 1,
                    fixes: 0,
                },
                FixCase {
                    // The parameter shadows the file's Effect binding where the fix would write Effect.failSync.
                    name: "shadowedEffect",
                    source: format!(
                        r#"{effect_import}
import {{ fail, suspend }} from "effect/Effect"
export const value = (Effect: {{ readonly name: string }}) => suspend(() => fail(Effect.name))
"#
                    ),
                    expected: String::new(),
                    matches: 1,
                    fixes: 0,
                },
            ];
            if version == EffectVersion::V4 {
                cases.extend([
                    FixCase {
                        name: "suspendFailToFailSync.ts",
                        source: String::new(),
                        expected: String::new(),
                        matches: 20,
                        fixes: 19,
                    },
                    FixCase {
                        name: "suspendFailToFailSync_preview.ts",
                        source: String::new(),
                        expected: String::new(),
                        matches: 1,
                        fixes: 1,
                    },
                    FixCase {
                        name: "suspendFailToFailSync_shapes.ts",
                        source: String::new(),
                        expected: String::new(),
                        matches: 15,
                        fixes: 14,
                    },
                ]);
            } else {
                cases.extend([
                    FixCase {
                        name: "suspendFailToFailSync.ts",
                        source: String::new(),
                        expected: String::new(),
                        matches: 25,
                        fixes: 24,
                    },
                    FixCase {
                        name: "suspendFailToFailSync_shapes.ts",
                        source: String::new(),
                        expected: String::new(),
                        matches: 18,
                        fixes: 17,
                    },
                    FixCase {
                        name: "dieSync",
                        source: format!(
                            r#"{effect_import}
export const value = Effect.suspend(() => Effect.die(new Error("late")))
"#
                        ),
                        expected: format!(
                            r#"{effect_import}
export const value = Effect.dieSync(() => new Error("late"))
"#
                        ),
                        matches: 1,
                        fixes: 1,
                    },
                ]);
            }
            let code = MESSAGE.code();
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
                    let mut source = test.source.clone();
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
                        // The fixture enables the rule as a warning; keep it a suggestion so
                        // VerifyNonSuggestionDiagnostics only sees compile errors.
                        source = fixture.replacen(
                            "suspendFailToFailSync:warning",
                            "suspendFailToFailSync:suggestion",
                            1,
                        );
                    }
                    let version_header = format!("// @{}\n", version.as_str());
                    if !source.starts_with(&version_header) {
                        source = format!("{version_header}{source}");
                    }
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
                            if !title.starts_with("Replace with Effect.") {
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
                    if matches != test.matches || fixes != test.fixes {
                        t.fatal(format!(
                            "got {matches} diagnostics and {fixes} compiled fixes, want {} and {}",
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
        "TestSuspendFailToFailSyncFixesCompile".to_string(),
        test_suspend_fail_to_fail_sync_fixes_compile,
    )]
}
