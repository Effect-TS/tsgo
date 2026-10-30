//! Port of `internal/rules/provide_merge_all_to_array_test.go`.

use ets_bundledeffect::EffectVersion;
use regex::Regex;

use crate::*;

// go test: internal/rules/provide_merge_all_to_array_test.go:13 TestProvideMergeAllToArrayShapes 940c4eb05ced2d1b
#[test]
fn test_provide_merge_all_to_array_shapes() {
    run_on_checker_stack(|t| {
        t.parallel();
        let source = r#"import { Context, Effect, Layer } from "effect"
class A extends Context.Tag("A")<A, { readonly a: number }>() {}
class B extends Context.Tag("B")<B, { readonly b: number }>() {}
class C extends Context.Tag("C")<C, { readonly c: number }>() {}
declare const ALayer: Layer.Layer<A>
declare const BLayer: Layer.Layer<B>
declare const CRequiresALayer: Layer.Layer<C, never, A>
declare const program: Effect.Effect<void, never, A | B>
export const effectFn = Effect.fn(function*() { yield* program }, Effect.provide(Layer.mergeAll(ALayer, BLayer)))
export const effectFnUntraced = Effect.fnUntraced(function*() { yield* program }, Effect.provide(Layer.mergeAll(ALayer, BLayer)))
export const unapplied = Effect.provide(Layer.mergeAll(ALayer, BLayer))
export const explicitMergeAllTypeArguments = program.pipe(Effect.provide(Layer.mergeAll<[Layer.Layer<A>, Layer.Layer<B>]>(ALayer, BLayer)))
export const explicitProvideTypeArguments = program.pipe(Effect.provide<A, never, never>(Layer.mergeAll(ALayer)))
// @ts-expect-error mergeAll requires at least one layer
export const noArguments = program.pipe(Effect.provide(Layer.mergeAll()))
export const dependentLayers = Effect.void.pipe(Effect.provide(Layer.mergeAll(ALayer, CRequiresALayer)))
"#;
        let want = ["effectFn", "effectFnUntraced", "unapplied"];
        let export_name = Regex::new(r"export const ([0-9A-Za-z_]+)").expect("valid regexp");

        for version in [EffectVersion::V3, EffectVersion::V4] {
            t.run(version.as_str(), |t| {
                t.parallel();
                // Only provideMergeAllToArray runs, so the dependentLayers skip does
                // not rely on layerMergeAllWithDependencies being enabled.
                let preview =
                    evaluate_preview(t, version, source, &ets_rules::PROVIDE_MERGE_ALL_TO_ARRAY);
                let mut got: Vec<String> = Vec::new();
                for diagnostic in &preview.diagnostics {
                    if !diagnostic.text.contains(
                        "Effect.provide accepts an array of layers and merges them itself",
                    ) {
                        t.fatal(format!("unexpected diagnostic text {:?}", diagnostic.text));
                    }
                    let span =
                        &preview.source_text[diagnostic.start as usize..diagnostic.end as usize];
                    if span != "Layer.mergeAll(ALayer, BLayer)" {
                        t.fatal(format!(
                            "diagnostic span {span:?} does not cover the Layer.mergeAll call"
                        ));
                    }
                    let start = diagnostic.start as usize;
                    let line_start =
                        preview.source_text[..start].rfind('\n').map_or(0, |index| index + 1);
                    let Some(m) = export_name.captures(&preview.source_text[line_start..start])
                    else {
                        t.fatal(format!(
                            "diagnostic at {} is not on an export line",
                            diagnostic.start
                        ));
                    };
                    got.push(m[1].to_string());
                }
                if got != want {
                    t.fatal(format!("matched {got:?}, want {want:?}"));
                }
            });
        }
    });
}
