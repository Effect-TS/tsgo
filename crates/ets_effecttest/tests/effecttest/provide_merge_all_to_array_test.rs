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

// go test: internal/effecttest/provide_merge_all_to_array_test.go:17 TestProvideMergeAllToArrayFixesCompile 45b47b8df0af5487
fn test_provide_merge_all_to_array_fixes_compile(t: &T) {
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
      "diagnosticSeverity": { "*": "off", "provideMergeAllToArray": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
"#;
            // The v3 fixture has no options cases: v3 Effect.provide takes no options.
            let mut main_matches = 19;
            if version == EffectVersion::V3 {
                main_matches = 17;
            }
            let cases = [
                Case {
                    name: "provideMergeAllToArray.ts",
                    source: "",
                    expected: "",
                    matches: main_matches,
                    fixes: main_matches,
                },
                Case {
                    name: "provideMergeAllToArray_preview.ts",
                    source: "",
                    expected: "",
                    matches: 1,
                    fixes: 1,
                },
                Case {
                    // A non-ASCII identifier before the call shifts byte offsets away
                    // from LSP positions.
                    name: "exactNonAsciiPrefix",
                    source: r#"import { Effect, Layer } from "effect"
declare const ALayer: Layer.Layer<"A">
declare const BLayer: Layer.Layer<"B">
declare const program: Effect.Effect<void, never, "A" | "B">
const café = "ü"
export const value = [café, program.pipe(Effect.provide(/* app */ Layer.mergeAll(ALayer, BLayer /* last */)))]
"#,
                    expected: r#"import { Effect, Layer } from "effect"
declare const ALayer: Layer.Layer<"A">
declare const BLayer: Layer.Layer<"B">
declare const program: Effect.Effect<void, never, "A" | "B">
const café = "ü"
export const value = [café, program.pipe(Effect.provide(/* app */ [ALayer, BLayer /* last */]))]
"#,
                    matches: 1,
                    fixes: 1,
                },
            ];
            let code = tsdiag::EFFECT_PROVIDE_ACCEPTS_AN_ARRAY_OF_LAYERS_AND_MERGES_THEM_ITSELF_PASS_THE_LAYERS_AS_AN_ARRAY_INSTEAD_OF_WRAPPING_THEM_IN_LAYER_MERGE_ALL_EFFECT_PROVIDE_MERGE_ALL_TO_ARRAY.code();
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
                            "provideMergeAllToArray:warning",
                            "provideMergeAllToArray:suggestion",
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
                            if title != "Pass the layers as an array to Effect.provide" {
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
        "TestProvideMergeAllToArrayFixesCompile".to_string(),
        test_provide_merge_all_to_array_fixes_compile,
    )]
}
