use ets_go::strconv::quote;
use ets_go::testing::on_checker_stack;

use crate::*;

// go test: internal/effecttest/inferred_generator_contextual_type_test.go:11 inferredGeneratorContextualTypeSource
// Rust: Go's `//go:embed testdata/inferredGeneratorContextualType.ts`, read in place.
const INFERRED_GENERATOR_CONTEXTUAL_TYPE_SOURCE: &str =
    include_str!("../../../internal/effecttest/testdata/inferredGeneratorContextualType.ts");

// go test: internal/effecttest/inferred_generator_contextual_type_test.go:13 TestIssue833InferredGeneratorNativeDiagnostics
#[test]
fn test_issue833_inferred_generator_native_diagnostics() {
    on_checker_stack(|| {
        ets_go::testing::run(|t| {
            t.parallel();

            const POSITIVE_CONTROL: &str = r"
interface Options {
  /** @stability experimental */
  experimental?: number;
  /** @stability unstable */
  unstable?: number;
}
declare function accept(options: Options): void;
export const contextual = function* () {
  accept({ experimental: 1, unstable: 2 });
};
";
            for enabled in [false, true] {
                t.run(&format!("rules={enabled}"), |t| {
                    t.parallel();
                    let mut severity = "off";
                    if enabled {
                        severity = "warning";
                    }
                    let content = format!(
                        r#"// @filename: tsconfig.json
{{
  "compilerOptions": {{
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "strict": true,
    "noEmit": true,
    "skipLibCheck": true,
    "plugins": [{{
      "name": "@effect/language-service",
      "diagnosticSeverity": {{
        "experimentalApiUsage": {},
        "unstableApiUsage": {},
        "lazyEffect": "off",
        "unnecessaryEffectGen": "off"
      }}
    }}]
  }}
}}
// @filename: package.json
{{ "type": "module" }}
// @filename: test.ts
"#,
                        quote(severity),
                        quote(severity)
                    ) + INFERRED_GENERATOR_CONTEXTUAL_TYPE_SOURCE
                        + POSITIVE_CONTROL;
                    let mut got = collect_diagnostic_strings_from_content(t, &content);
                    got.sort();
                    let mut want: Vec<String> = Vec::new();
                    if enabled {
                        want = vec![
                            "TS377135: `experimental` is an experimental API. effect(experimentalApiUsage)".to_string(),
                            "TS377136: `unstable` is an unstable API. Breaking changes may happen between versions. effect(unstableApiUsage)".to_string(),
                        ];
                    }
                    if got != want {
                        t.fatal(format!("diagnostics = {got:?}, want {want:?}"));
                    }
                });
            }
        });
    });
}
