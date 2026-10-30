// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use std::collections::BTreeMap;

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::diagnostics as tsdiag;
use ets_shim::ls::lsconv;
use libtest_mimic::Trial;

use crate::{has_diagnostic_code, run_case};

// go test: internal/effecttest/effect_succeed_with_undefined_test.go:17 TestEffectSucceedWithUndefinedFixesCompile c2ebd5b4130d8987
//
// Rust: Go's `sources` map runs its subtests in random order; a `BTreeMap` runs them by name.
fn test_effect_succeed_with_undefined_fixes_compile(t: &T) {
    ets_effecttest::init_hooks();
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
      "diagnosticSeverity": { "*": "off", "effectSucceedWithUndefined": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
"#;
    let mut sources: BTreeMap<String, String> = BTreeMap::from([
        (
            "callShapes".to_string(),
            r#"import { Effect, pipe } from "effect"
export const dataFirst = Effect.andThen(Effect.void, Effect.succeed(undefined))
export const pipeSubject = pipe(Effect.succeed(undefined), Effect.as(1))
export const fn = Effect.fn(function*() { return yield* Effect.succeed(undefined) })
export const fnUntraced = Effect.fnUntraced(function*() { return yield* Effect.succeed(undefined) })
"#
            .to_string(),
        ),
        (
            "namedSucceedOnly".to_string(),
            r#"import { succeed } from "effect/Effect"
// NO replacement quickfix: no binding reaches Effect.undefined
export const result = succeed(undefined)
"#
            .to_string(),
        ),
        (
            "namedUndefinedImport".to_string(),
            r#"import { succeed, undefined as undefinedEffect } from "effect/Effect"
export const result = succeed(undefined)
export const used = undefinedEffect
"#
            .to_string(),
        ),
    ]);
    for name in [
        "effectSucceedWithUndefined.ts",
        "effectSucceedWithUndefined_aliases.ts",
        "effectSucceedWithUndefined_preview.ts",
        "effectSucceedWithUndefined_namedImport.ts",
        "effectSucceedWithUndefined_scopedEffect.ts",
    ] {
        let fixture = match std::fs::read_to_string(
            ets_effecttest::test_data_path()
                .join("tests")
                .join(EffectVersion::V4.as_str())
                .join(name),
        ) {
            Ok(fixture) => fixture,
            Err(err) => t.fatal(err.to_string()),
        };
        sources.insert(name.to_string(), fixture);
    }
    let code = tsdiag::EFFECT_UNDEFINED_REPRESENTS_THE_SAME_OUTCOME_AS_EFFECT_SUCCEED_UNDEFINED_OR_EFFECT_SUCCEED_VOID_0_EFFECT_EFFECT_SUCCEED_WITH_UNDEFINED.code();
    for (name, source) in &sources {
        t.run(name, |t| {
            t.parallel();
            let mut f = ets_fourslash::new_fourslash(t, None, &format!("{PREFIX}{source}"));
            f.verify_non_suggestion_diagnostics(t, &[]);
            let uri = lsconv::file_name_to_document_uri("/repro.ts");
            let lines: Vec<&str> = source.split('\n').collect();
            let (mut checked, mut diagnostics) = (0, 0);
            for diagnostic in &f.get_quick_fixes_for_diagnostics(t, &uri) {
                if !has_diagnostic_code(&diagnostic.diagnostic, code) {
                    continue;
                }
                let line = lines[diagnostic.diagnostic.range.start.line as usize];
                let want_no_rewrite = comment_above_contains(
                    &lines,
                    i64::from(diagnostic.diagnostic.range.start.line),
                    "NO replacement quickfix",
                );
                let mut offered = false;
                for (index, title) in diagnostic.fix_titles.iter().enumerate() {
                    if title != "Replace with Effect.undefined" {
                        continue;
                    }
                    offered = true;
                    let result = f.apply_quick_fix(t, &uri, &diagnostic.diagnostic, index);
                    if result.changes.len() != 1 {
                        t.fatal(format!("expected one changed file, got {}", result.changes.len()));
                    }
                    {
                        let mut converted = ets_fourslash::new_fourslash(
                            t,
                            None,
                            &format!("{PREFIX}{}", result.changes[0].after),
                        );
                        converted.verify_non_suggestion_diagnostics(t, &[]);
                        converted.done(t);
                    }
                    checked += 1;
                }
                if offered == want_no_rewrite {
                    t.fatal(format!(
                        "line {line:?}: replacement offered = {offered}, want {}",
                        !want_no_rewrite
                    ));
                }
                diagnostics += 1;
            }
            if diagnostics == 0 {
                t.fatal("no effectSucceedWithUndefined diagnostics were checked");
            }
            t.log(format!("compiled {checked} replacements"));
            f.done(t);
        });
    }
}

// go test: internal/effecttest/effect_succeed_with_undefined_test.go:105 commentAboveContains 50a28d90718769cd
// commentAboveContains reports whether the lines of line's case, up to the previous blank line, contain marker.
fn comment_above_contains(lines: &[&str], line: i64, marker: &str) -> bool {
    let mut i = line - 1;
    while i >= 0 && !lines[i as usize].trim().is_empty() {
        if lines[i as usize].contains(marker) {
            return true;
        }
        i -= 1;
    }
    false
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case(
        "TestEffectSucceedWithUndefinedFixesCompile".to_string(),
        test_effect_succeed_with_undefined_fixes_compile,
    )]
}
