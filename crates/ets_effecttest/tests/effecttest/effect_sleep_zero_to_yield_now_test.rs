// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::diagnostics as tsdiag;
use ets_shim::ls::lsconv;
use libtest_mimic::Trial;

use crate::{has_diagnostic_code, run_case};

// go test: internal/effecttest/effect_sleep_zero_to_yield_now_test.go:16 TestEffectSleepZeroToYieldNowFixesCompile
fn test_effect_sleep_zero_to_yield_now_fixes_compile(t: &T) {
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
      "diagnosticSeverity": { "*": "off", "effectSleepZeroToYieldNow": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
"#;
    ets_effecttest::init_hooks();
    t.parallel();
    for version in [EffectVersion::V3, EffectVersion::V4] {
        t.run(version.as_str(), |t| {
            t.parallel();
            for name in [
                "effectSleepZeroToYieldNow.ts",
                "effectSleepZeroToYieldNow_aliases.ts",
                "effectSleepZeroToYieldNow_preview.ts",
            ] {
                let fixture = match std::fs::read_to_string(
                    ets_effecttest::test_data_path()
                        .join("tests")
                        .join(version.as_str())
                        .join(name),
                ) {
                    Ok(fixture) => fixture,
                    Err(err) => t.fatal(err.to_string()),
                };
                let mut f = ets_fourslash::new_fourslash(t, None, &format!("{PREFIX}{fixture}"));
                f.verify_non_suggestion_diagnostics(t, &[]);
                let uri = lsconv::file_name_to_document_uri("/repro.ts");
                let inventory = f.get_quick_fixes_for_diagnostics(t, &uri);
                let code = tsdiag::CONSIDER_EFFECT_YIELD_NOW_FOR_COOPERATIVE_YIELDING_INSTEAD_OF_SLEEPING_FOR_ZERO_DURATION_EFFECT_EFFECT_SLEEP_ZERO_TO_YIELD_NOW.code();
                let mut suggestions = 0;
                for diagnostic in &inventory {
                    if has_diagnostic_code(&diagnostic.diagnostic, code) {
                        suggestions += 1;
                    }
                }
                let mut checked = 0;
                for diagnostic in &inventory {
                    for (index, title) in diagnostic.fix_titles.iter().enumerate() {
                        if title != "Replace with Effect.yieldNow" {
                            continue;
                        }
                        let result = f.apply_quick_fix(t, &uri, &diagnostic.diagnostic, index);
                        if result.changes.len() != 1 {
                            t.fatal(format!(
                                "{name}: expected one changed file, got {}",
                                result.changes.len()
                            ));
                        }
                        {
                            let mut converted = ets_fourslash::new_fourslash(
                                t,
                                None,
                                &format!("{PREFIX}{}", result.changes[0].after),
                            );
                            converted.verify_non_suggestion_diagnostics(t, &[]);
                            let remaining = converted.get_quick_fixes_for_diagnostics(t, &uri);
                            let mut count = 0;
                            for diagnostic in &remaining {
                                if has_diagnostic_code(&diagnostic.diagnostic, code) {
                                    count += 1;
                                }
                            }
                            if count != suggestions - 1 {
                                t.fatal(format!(
                                    "{name}: expected {} sleep-zero diagnostics after replacement, got {count}",
                                    suggestions - 1
                                ));
                            }
                            converted.done(t);
                        }
                        checked += 1;
                    }
                }
                f.done(t);
                if checked == 0 {
                    t.fatal(format!("{name}: no yieldNow replacements were checked"));
                }
                t.log(format!("{name}: compiled {checked} replacements"));
            }
        });
    }
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case(
        "TestEffectSleepZeroToYieldNowFixesCompile".to_string(),
        test_effect_sleep_zero_to_yield_now_fixes_compile,
    )]
}
