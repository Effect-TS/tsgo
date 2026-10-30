// Rust: Go's blank imports of `etscheckerhooks`, `etslshooks` and `etstesthooks`: each test registers them
// (`ets_effecttest::init_hooks`).

use ets_go::testing::T;
use ets_shim::ls::lsconv;
use libtest_mimic::Trial;

use crate::run_case;

// go test: internal/effecttest/feature_flags_test.go:14 TestEffectRefactorsDisabled 65549802b44e3440
fn test_effect_refactors_disabled(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let content = r#"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service",
        "refactors": false
      }
    ]
  }
}
// @Filename: /test.ts
import * as Eff from "effect/Effect"

export const program = () =>
  [|Eff.gen(function*() {
    const a = yield* Eff.succeed(1)
    const b = yield* Eff.succeed(2)
    return a + b
  })|]"#;

    let mut f = ets_fourslash::new_fourslash(t, None, content);

    let ranges = f.ranges();
    if ranges.len() != 1 {
        t.fatal(format!("expected exactly one range, got {}", ranges.len()));
    }
    let actions = f.get_refactor_actions_for_range(
        t,
        &lsconv::file_name_to_document_uri(ranges[0].file_name()),
        ranges[0].ls_range,
    );
    if !actions.is_empty() {
        t.fatal(format!(
            "expected no refactor actions when refactors=false, got {}",
            actions.len()
        ));
    }
    f.done(t);
}

// go test: internal/effecttest/feature_flags_test.go:55 TestEffectOverridesCanDisableMatchingFile ada558ca65f3dae3
fn test_effect_overrides_can_disable_matching_file(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let content = r#"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "floatingEffect": "error"
        },
        "overrides": [
          {
            "include": ["src/**/*.ts"],
            "options": {
              "diagnosticSeverity": {
                "floatingEffect": "off"
              }
            }
          }
        ]
      }
    ]
  }
}
// @Filename: /src/test.ts
import { Effect } from "effect"

Effect.succeed(1)"#;

    let mut f = ets_fourslash::new_fourslash(t, None, content);

    f.verify_diagnostics(t, &[]);
    f.done(t);
}

// go test: internal/effecttest/feature_flags_test.go:96 TestEffectOverridesCanOverrideRuleOptions 4aa90dd9c7542445
fn test_effect_overrides_can_override_rule_options(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let content = r#"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "missedPipeableOpportunity": "error"
        },
        "pipeableMinArgCount": 2,
        "overrides": [
          {
            "include": ["src/**/*.ts"],
            "options": {
              "pipeableMinArgCount": 3
            }
          }
        ]
      }
    ]
  }
}
// @Filename: /src/test.ts
import { identity, Schema } from "effect"

const User = Schema.Struct({ id: Schema.Number })
export const preview = identity(identity(Schema.decodeEffect(User)({ id: 1 })))"#;

    let mut f = ets_fourslash::new_fourslash(t, None, content);

    f.verify_diagnostics(t, &[]);
    f.done(t);
}

// go test: internal/effecttest/feature_flags_test.go:137 TestEffectOverridesPreserveExtendedConfigBasePaths c61cee8d1318c385
fn test_effect_overrides_preserve_extended_config_base_paths(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let content = r#"// @Filename: /configs/base.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "overrides": [
          {
            "include": ["shared/**/*.ts"],
            "options": {
              "diagnosticSeverity": {
                "floatingEffect": "off"
              }
            }
          }
        ]
      }
    ]
  }
}
// @Filename: /tsconfig.json
{
  "extends": "./configs/base.json",
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "floatingEffect": "error"
        }
      }
    ]
  }
}
// @Filename: /configs/shared/test.ts
import { Effect } from "effect"

Effect.succeed(1)"#;

    let mut f = ets_fourslash::new_fourslash(t, None, content);

    f.verify_diagnostics(t, &[]);
    f.done(t);
}

// go test: internal/effecttest/feature_flags_test.go:189 TestEffectDiagnosticsWorkThroughMultiHopExtends 326751a85f7ba31f
fn test_effect_diagnostics_work_through_multi_hop_extends(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let content = r#"// @Filename: /base.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "floatingEffect": "error"
        }
      }
    ]
  }
}
// @Filename: /worker.json
{
  "extends": "./base.json",
  "compilerOptions": {
    "jsx": "react-jsx"
  }
}
// @Filename: /tsconfig.json
{
  "extends": "./worker.json",
  "include": ["./index.ts"]
}
// @Filename: /index.ts
import { Effect } from "effect"

[|Effect.succeed(1)|]"#;

    let mut f = ets_fourslash::new_fourslash(t, None, content);

    let ranges = f.ranges();
    if ranges.len() != 1 {
        t.fatal(format!("expected exactly one range, got {}", ranges.len()));
    }
    f.verify_error_exists_at_range(t, &ranges[0], 377_001, "");
    f.done(t);
}

/// Rust-only: the Go test functions of `feature_flags_test.go`, one trial each.
pub(crate) fn tests() -> Vec<Trial> {
    vec![
        run_case("TestEffectRefactorsDisabled".to_string(), test_effect_refactors_disabled),
        run_case(
            "TestEffectOverridesCanDisableMatchingFile".to_string(),
            test_effect_overrides_can_disable_matching_file,
        ),
        run_case(
            "TestEffectOverridesCanOverrideRuleOptions".to_string(),
            test_effect_overrides_can_override_rule_options,
        ),
        run_case(
            "TestEffectOverridesPreserveExtendedConfigBasePaths".to_string(),
            test_effect_overrides_preserve_extended_config_base_paths,
        ),
        run_case(
            "TestEffectDiagnosticsWorkThroughMultiHopExtends".to_string(),
            test_effect_diagnostics_work_through_multi_hop_extends,
        ),
    ]
}
