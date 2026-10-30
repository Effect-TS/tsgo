use ets_go::{sprintf, sprintln};
use ets_shim::checker::Type;
use ets_shim::core::P;

use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/location_independent_type_parser_test.go:10 TestParsersAcceptInstantiatedV3TypesWithoutLocation c70b9025e4d1ee19
#[test]
fn test_parsers_accept_instantiated_v3_types_without_location() {
    type Parse = fn(&mut TypeParser<'_>, Option<P<Type>>) -> Option<Vec<P<Type>>>;

    run(|t| {
        t.parallel();
        if let Err(err) = ensure_package_installed(EFFECT_V3, "effect") {
            t.skip(sprintln!("Effect v3 not installed:", err));
        }

        let cases: [(&str, &str, Parse, &[&str]); 4] = [
            (
                "Effect",
                r#"import * as Effect from "effect/Effect"
declare const value: Effect.Effect<string, Error, number>"#,
                |tp: &mut TypeParser<'_>, typ| {
                    tp.effect_type(typ).map(|parsed| vec![parsed.a, parsed.e, parsed.r])
                },
                &["string", "Error", "number"],
            ),
            (
                "Layer",
                r#"import * as Layer from "effect/Layer"
declare const value: Layer.Layer<string, Error, number>"#,
                |tp: &mut TypeParser<'_>, typ| {
                    tp.layer_type(typ).map(|parsed| vec![parsed.r_out, parsed.e, parsed.r_in])
                },
                &["string", "Error", "number"],
            ),
            (
                "Schema",
                r#"import * as Schema from "effect/Schema"
declare const value: Schema.Schema<string, number>"#,
                |tp: &mut TypeParser<'_>, typ| {
                    tp.effect_schema_types(typ).map(|parsed| vec![parsed.a, parsed.e])
                },
                &["string", "number"],
            ),
            (
                "Stream",
                r#"import * as Stream from "effect/Stream"
declare const value: Stream.Stream<string, Error, number>"#,
                |tp: &mut TypeParser<'_>, typ| {
                    tp.stream_type(typ).map(|parsed| vec![parsed.a, parsed.e, parsed.r])
                },
                &["string", "Error", "number"],
            ),
        ];
        for (name, decl, parse, want) in cases {
            t.run(name, |t| {
                t.parallel();
                let (mut compiled, sf) =
                    compile_and_get_checker_and_source_file_with_effect_v3_internal(
                        t,
                        &(decl.to_string() + "\nvalue\n"),
                    );
                let mut tp = compiled.type_parser();

                let typ = tp.get_type_at_location(find_identifier_by_text(t, sf, "value", 1));
                let Some(parts) = parse(&mut tp, typ) else {
                    t.fatal(sprintf!(
                        "expected instantiated v3 %s type to parse without a location",
                        name
                    ));
                };
                for (i, &want) in want.iter().enumerate() {
                    let got = type_string(&mut tp, parts[i]);
                    if got != want {
                        t.error(sprintf!(
                            "type argument %d: expected %s, got %s",
                            i as i64,
                            want,
                            got
                        ));
                    }
                }
            });
        }
    });
}
