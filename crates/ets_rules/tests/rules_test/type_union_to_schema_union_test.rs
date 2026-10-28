//! Port of `internal/rules/type_union_to_schema_union_test.go`.

use ets_bundledeffect::EffectVersion;
use rustc_hash::FxHashMap;

use crate::*;

// go test: internal/rules/type_union_to_schema_union_test.go:12 TestTypeUnionToSchemaUnionMatches e40010dd3d14a3cd
#[test]
fn test_type_union_to_schema_union_matches() {
    run_on_checker_stack(|t| {
        t.parallel();
        let source = r#"import { Schema } from "effect"
export const A = Schema.Struct({ a: Schema.String })
export const B = Schema.Struct({ b: Schema.String })
export type TypeQueries = typeof A.Type | typeof B.Type
export type EncodedQueries = typeof A.Encoded | typeof B.Encoded
export type Nested = ((typeof A.Type) | (typeof B.Type | typeof A.Type))
export type TypeHelpers = Schema.Schema.Type<typeof A> | typeof B.Type
export type V3EncodedHelpers = Schema.Schema.Encoded<typeof A> | typeof B.Encoded
export type V4EncodedHelpers = Schema.Codec.Encoded<typeof A> | typeof B.Encoded
export type Late = typeof A.Type | typeof C.Type
export const C = Schema.Struct({ c: Schema.String })
export type Single = typeof A.Type
export type MixedSides = typeof A.Type | typeof B.Encoded
export type WithNull = typeof A.Type | typeof B.Type | null
export type ContextHelpers = Schema.Schema.Context<typeof A> | Schema.Schema.Context<typeof B>
export type Generic<_T> = typeof A.Type | typeof B.Type
declare const notSchema: { readonly Type: string; readonly Encoded: string }
export type NotSchema = typeof A.Type | typeof notSchema.Type
"#;
        let sides: FxHashMap<&str, &str> = FxHashMap::from_iter([
            ("TypeQueries", "Type"),
            ("EncodedQueries", "Encoded"),
            ("Nested", "Type"),
            ("TypeHelpers", "Type"),
            ("V3EncodedHelpers", "Encoded"),
            ("V4EncodedHelpers", "Encoded"),
            ("Late", "Type"),
        ]);

        for version in [EffectVersion::V3, EffectVersion::V4] {
            t.run(version.as_str(), |t| {
                t.parallel();
                let mut want = vec![
                    "TypeQueries",
                    "EncodedQueries",
                    "Nested",
                    "TypeHelpers",
                    "V3EncodedHelpers",
                    "V4EncodedHelpers",
                    "Late",
                ];
                // Encoded helpers only exist as Schema.Schema.Encoded in v3 and as
                // Schema.Codec.Encoded in v4.
                let mut unsupported = "V4EncodedHelpers";
                if version == EffectVersion::V4 {
                    unsupported = "V3EncodedHelpers";
                }
                want.retain(|name| *name != unsupported);

                let preview =
                    evaluate_preview(t, version, source, &ets_rules::TYPE_UNION_TO_SCHEMA_UNION);
                let mut got: Vec<String> = Vec::new();
                for diagnostic in &preview.diagnostics {
                    let name =
                        &preview.source_text[diagnostic.start as usize..diagnostic.end as usize];
                    let Some(side) = sides.get(name) else {
                        t.fatal(format!("unexpected diagnostic on {name:?}"));
                    };
                    if !diagnostic.text.contains(&format!("unions Schema `.{side}` types")) {
                        t.fatal(format!(
                            "{name}: unexpected diagnostic text {:?}",
                            diagnostic.text
                        ));
                    }
                    got.push(name.to_string());
                }
                if got != want {
                    t.fatal(format!("matched {got:?}, want {want:?}"));
                }
            });
        }
    });
}
