use ets_go::testing::run;

use crate::*;

// go test: internal/rules/prefer_effect_record_test.go:5 TestPreferEffectRecordInheritedAugmentation e70557c50149b0e6
#[test]
fn test_prefer_effect_record_inherited_augmentation() {
    run(|t| {
        t.parallel();
        let diagnostics = compile_api_stability_files(
            t,
            &files(&[(
                "test.ts",
                r#"
export {};
declare global { interface Object extends Pick<ObjectConstructor, "keys"> {} }
Object.keys({ a: 1 });
"#,
            )]),
        )
        .with_rule_ctx(|ctx| {
            run_prefer_effect_record(ctx).iter().map(|d| d.localize()).collect::<Vec<_>>()
        });
        const WANT: &str = "Consider effect/Record.keys instead of native Object.keys. Requires a record rather than arrays or primitives; types keys as the inferred key union, which does not guarantee exact runtime keys. effect(preferEffectRecord)";
        if diagnostics.len() != 1 || diagnostics[0] != WANT {
            t.fatal(format!("diagnostics = {diagnostics:?}, want one {WANT:?}"));
        }
    });
}

// go test: internal/rules/prefer_effect_record_test.go:20 TestPreferEffectRecordRemappedMember a13614f3d767f7b5
#[test]
fn test_prefer_effect_record_remapped_member() {
    run(|t| {
        t.parallel();
        let diagnostics = compile_api_stability_files(
            t,
            &files(&[(
                "test.ts",
                r#"
type Remapped = { [K in keyof ObjectConstructor as K extends "values" ? "keys" : never]: ObjectConstructor[K] };
declare const remapped: Remapped;
remapped.keys;
"#,
            )])).with_rule_ctx(|ctx| run_prefer_effect_record(ctx).iter().map(|d| d.localize()).collect::<Vec<_>>());
        const WANT: &str = "Consider effect/Record.values instead of native Object.values. Requires a record rather than arrays or primitives; snapshots keys before reads, so getter or proxy mutations can change the result. effect(preferEffectRecord)";
        if diagnostics.len() != 1 || diagnostics[0] != WANT {
            t.fatal(format!("diagnostics = {diagnostics:?}, want one {WANT:?}"));
        }
    });
}
