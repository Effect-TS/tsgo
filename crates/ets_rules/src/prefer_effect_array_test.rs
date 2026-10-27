use ets_go::testing::run;

use crate::*;

// go test: internal/rules/prefer_effect_array_test.go:5 TestPreferEffectArrayInheritedAugmentation 616d3400e73c6bcc
#[test]
fn test_prefer_effect_array_inherited_augmentation() {
    run(|t| {
        t.parallel();
        let diagnostics = compile_api_stability_files(
            t,
            &files(&[(
                "test.ts",
                r#"
export {};
declare global {
  interface Array<T> extends Pick<ArrayConstructor, "isArray"> {}
}
Array.isArray([]);
"#,
            )]),
        )
        .with_rule_ctx(|ctx| {
            run_prefer_effect_array(ctx).iter().map(|d| d.localize()).collect::<Vec<_>>()
        });
        const WANT: &str = "Consider effect/Array.isArray instead of native Array.isArray. Narrows elements to unknown rather than any. effect(preferEffectArray)";
        if diagnostics.len() != 1 || diagnostics[0] != WANT {
            t.fatal(format!("diagnostics = {diagnostics:?}, want one {WANT:?}"));
        }
    });
}

// go test: internal/rules/prefer_effect_array_test.go:22 TestPreferEffectArrayRemappedMember 7fd58678b6d06256
#[test]
fn test_prefer_effect_array_remapped_member() {
    run(|t| {
        t.parallel();
        let diagnostics = compile_api_stability_files(
            t,
            &files(&[(
                "test.ts",
                r#"
type Remapped = { [K in keyof number[] as K extends "filter" ? "map" : never]: number[][K] };
declare const remapped: Remapped;
remapped.map;
"#,
            )]),
        )
        .with_rule_ctx(|ctx| {
            run_prefer_effect_array(ctx).iter().map(|d| d.localize()).collect::<Vec<_>>()
        });
        const WANT: &str = "Consider effect/Array.filter instead of native Array.filter. The callback type accepts (value, index), with no array parameter or thisArg; sparse-array holes are visited. effect(preferEffectArray)";
        if diagnostics.len() != 1 || diagnostics[0] != WANT {
            t.fatal(format!("diagnostics = {diagnostics:?}, want one {WANT:?}"));
        }
    });
}
