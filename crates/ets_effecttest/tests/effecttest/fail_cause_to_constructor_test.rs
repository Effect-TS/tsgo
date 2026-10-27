// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::diagnostics as tsdiag;
use ets_shim::ls::lsconv;
use libtest_mimic::Trial;

use crate::{has_diagnostic_code, run_case};

/// Rust-only: Go's function-local `fixCase` type.
struct FixCase {
    name: &'static str,
    source: &'static str,
    expected: &'static str,
    matches: i32,
    fixes: i32,
}

// go test: internal/effecttest/fail_cause_to_constructor_test.go:17 TestFailCauseToConstructorFixesCompile
fn test_fail_cause_to_constructor_fixes_compile(t: &T) {
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
      "diagnosticSeverity": { "*": "off", "failCauseToConstructor": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
"#;
            let mut cases = vec![
                FixCase {
                    name: "pipePrefix",
                    source: r#"import { Cause, Effect, pipe } from "effect"
declare const defect: unknown
export const value = pipe(Cause.die(defect), Effect.failCause, Effect.asVoid)
"#,
                    expected: r#"import { Cause, Effect, pipe } from "effect"
declare const defect: unknown
export const value = pipe(Effect.die(defect), Effect.asVoid)
"#,
                    matches: 1,
                    fixes: 1,
                },
                FixCase {
                    name: "namedImportsLazy",
                    source: r#"import { Effect } from "effect"
import { fail as causeFail } from "effect/Cause"
import { failCauseSync } from "effect/Effect"
declare const error: Error
export const value = failCauseSync(() => causeFail(error))
"#,
                    expected: r#"import { Effect } from "effect"
import { fail as causeFail } from "effect/Cause"
import { failCauseSync } from "effect/Effect"
declare const error: Error
export const value = Effect.failSync(() => error)
"#,
                    matches: 1,
                    fixes: 1,
                },
                FixCase {
                    name: "lazyBlock",
                    source: r#"import { Cause, Effect } from "effect"
declare const error: Error
export const value = Effect.failCauseSync(() => {
  return Cause.fail(error)
})
"#,
                    expected: r#"import { Cause, Effect } from "effect"
declare const error: Error
export const value = Effect.failSync(() => {
  return error
})
"#,
                    matches: 1,
                    fixes: 1,
                },
                FixCase {
                    // A bare object literal as a concise arrow body would parse as a block.
                    name: "lazyObjectLiteral",
                    source: r#"import { Cause, Effect } from "effect"
export const value = Effect.failCauseSync(() => Cause.fail({ _tag: "E" }))
"#,
                    expected: r#"import { Cause, Effect } from "effect"
export const value = Effect.failSync(() => ({ _tag: "E" }))
"#,
                    matches: 1,
                    fixes: 1,
                },
                FixCase {
                    // Cause.fail widens the literal union to string; a failSync thunk would
                    // keep "A" | "B", so the fix is withheld.
                    name: "lazyLiteralUnion",
                    source: r#"import { Cause, Effect } from "effect"
declare const condition: boolean
export const value = Effect.failCauseSync(() => Cause.fail(condition ? "A" : "B"))
"#,
                    expected: "",
                    matches: 1,
                    fixes: 0,
                },
                FixCase {
                    // Without an Effect module binding the fallback receiver would be unbound.
                    name: "namedImportsOnly",
                    source: r#"import { die as causeDie, fail as causeFail } from "effect/Cause"
import { failCause, failCauseSync } from "effect/Effect"
declare const error: Error
export const eager = failCause(causeDie(error))
export const lazy = failCauseSync(() => causeFail(error))
"#,
                    expected: "",
                    matches: 2,
                    fixes: 0,
                },
                FixCase {
                    // A type-only Effect binding is not a value; Effect.fail would be TS1361.
                    name: "typeOnlyEffect",
                    source: r#"import type { Effect } from "effect"
import { fail as causeFail } from "effect/Cause"
import { failCause, failCauseSync } from "effect/Effect"
declare const error: Error
export const eager: Effect.Effect<never, Error> = failCause(causeFail(error))
export const lazy: Effect.Effect<never, Error> = failCauseSync(() => causeFail(error))
"#,
                    expected: "",
                    matches: 2,
                    fixes: 0,
                },
                FixCase {
                    // The lazy form as a pipe subject is reported without a fix.
                    name: "lazyFunctionPipe",
                    source: r#"import { Cause, Effect, pipe } from "effect"
declare const error: Error
export const value = pipe(() => Cause.fail(error), Effect.failCauseSync)
"#,
                    expected: "",
                    matches: 1,
                    fixes: 0,
                },
            ];
            if version == EffectVersion::V4 {
                cases.extend([
                    FixCase {
                        name: "failCauseToConstructor.ts",
                        source: "",
                        expected: "",
                        matches: 24,
                        fixes: 22,
                    },
                    FixCase {
                        name: "failCauseToConstructor_preview.ts",
                        source: "",
                        expected: "",
                        matches: 1,
                        fixes: 1,
                    },
                ]);
            } else {
                cases.extend([
                    FixCase {
                        name: "failCauseToConstructor.ts",
                        source: "",
                        expected: "",
                        matches: 24,
                        fixes: 21,
                    },
                    FixCase {
                        name: "lazyDie",
                        source: r#"import * as Cause from "effect/Cause"
import * as Effect from "effect/Effect"
export const value = Effect.failCauseSync(() => Cause.die(new Error("late")))
"#,
                        expected: r#"import * as Cause from "effect/Cause"
import * as Effect from "effect/Effect"
export const value = Effect.dieSync(() => new Error("late"))
"#,
                        matches: 1,
                        fixes: 1,
                    },
                ]);
            }
            cases.extend([
                FixCase {
                    name: "failCauseToConstructor_namedImports.ts",
                    source: "",
                    expected: "",
                    matches: 3,
                    fixes: 0,
                },
                FixCase {
                    name: "failCauseToConstructor_aliasedEffect.ts",
                    source: "",
                    expected: "",
                    matches: 2,
                    fixes: 2,
                },
            ]);
            let code = tsdiag::EFFECT_0_EXPRESSES_THIS_FAILURE_DIRECTLY_INSTEAD_OF_WRAPPING_CAUSE_2_IN_EFFECT_1_EFFECT_FAIL_CAUSE_TO_CONSTRUCTOR.code();
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
                            "failCauseToConstructor:warning",
                            "failCauseToConstructor:suggestion",
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
        "TestFailCauseToConstructorFixesCompile".to_string(),
        test_fail_cause_to_constructor_fixes_compile,
    )]
}
