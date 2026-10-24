use ets_go::testing::on_checker_stack;

use crate::*;

// go test: internal/effecttest/effect_in_void_success_ts2589_test.go:5 TestIssue381EffectInVoidSuccessDoesNotReportExcessiveTypeDepth
#[test]
fn test_issue381_effect_in_void_success_does_not_report_excessive_type_depth() {
    on_checker_stack(|| {
        ets_go::testing::run(|t| {
            t.parallel();

            let without_plugin = collect_diagnostic_strings_from_content(
                t,
                &build_issue381_effect_in_void_success_case(false),
            );
            if has_diagnostic_code(&without_plugin, "TS2589:") {
                t.fatal(format!("did not expect TS2589 without plugin, got {without_plugin:?}"));
            }

            let with_plugin = collect_diagnostic_strings_from_content(
                t,
                &build_issue381_effect_in_void_success_case(true),
            );
            if has_diagnostic_code(&with_plugin, "TS2589:") {
                t.fatal(format!(
                    "did not expect effectInVoidSuccess to introduce TS2589, got {with_plugin:?}"
                ));
            }
        });
    });
}

// go test: internal/effecttest/effect_in_void_success_ts2589_test.go:19 buildIssue381EffectInVoidSuccessCase
fn build_issue381_effect_in_void_success_case(plugin_enabled: bool) -> String {
    let mut plugin_config = "";
    if plugin_enabled {
        plugin_config = r#",
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "effectInVoidSuccess": "warning"
        }
      }
    ]"#;
    }

    r#"// @filename: tsconfig.json
{
  "compilerOptions": {
    "target": "ES2025",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "noEmit": true,
    "skipLibCheck": true,
    "strict": true"#
        .to_string()
        + plugin_config
        + r#"
  }
}

// @filename: test.ts
import type { Schema } from "effect"

type Recurse<Value> = {
  [Key in keyof Value]: Recurse<Value[Key]>
}

type Json = readonly Json[] | JsonObject
type JsonObject = { readonly [key: string]: Json }

type ValueType<Definition> =
  Definition extends Schema.Codec<infer Value, Json> ? Value : never

declare let value: Recurse<
  ValueType<Schema.Codec<JsonObject>> | null
>

value = null
"#
}
