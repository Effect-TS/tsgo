use ets_go::{sprintf, sprintln};

use crate::helpers_test::*;

// go test: internal/typeparser/location_independent_type_parser_test.go:9 TestEffectTypeParsesInstantiatedV3TypeWithoutLocation
#[test]
fn test_effect_type_parses_instantiated_v3_type_without_location() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_package_installed(EFFECT_V3, "effect") {
            t.skip(sprintln!("Effect v3 not installed:", err));
        }

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_with_effect_v3_internal(
            t,
            r#"
import * as Effect from "effect/Effect"

declare const effect: Effect.Effect<string, Error, number>
effect
"#,
        );
        let mut tp = compiled.type_parser();

        let effect_type = tp.get_type_at_location(find_identifier_by_text(t, sf, "effect", 1));
        let Some(parsed) = tp.effect_type(effect_type) else {
            t.fatal("expected instantiated v3 Effect type to parse without a location");
        };
        let got = type_string(&mut tp, parsed.a);
        if got != "string" {
            t.fatal(sprintf!("expected success type string, got %s", got));
        }
        let got = type_string(&mut tp, parsed.e);
        if got != "Error" {
            t.fatal(sprintf!("expected error type Error, got %s", got));
        }
        let got = type_string(&mut tp, parsed.r);
        if got != "number" {
            t.fatal(sprintf!("expected requirements type number, got %s", got));
        }
    });
}

// go test: internal/typeparser/location_independent_type_parser_test.go:39 TestLayerTypeParsesInstantiatedV3TypeWithoutLocation
#[test]
fn test_layer_type_parses_instantiated_v3_type_without_location() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_package_installed(EFFECT_V3, "effect") {
            t.skip(sprintln!("Effect v3 not installed:", err));
        }

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_with_effect_v3_internal(
            t,
            r#"
import * as Layer from "effect/Layer"

declare const layer: Layer.Layer<string, Error, number>
layer
"#,
        );
        let mut tp = compiled.type_parser();

        let layer_type = tp.get_type_at_location(find_identifier_by_text(t, sf, "layer", 1));
        let Some(parsed) = tp.layer_type(layer_type) else {
            t.fatal("expected instantiated v3 Layer type to parse without a location");
        };
        let got = type_string(&mut tp, parsed.r_out);
        if got != "string" {
            t.fatal(sprintf!("expected output requirements type string, got %s", got));
        }
        let got = type_string(&mut tp, parsed.e);
        if got != "Error" {
            t.fatal(sprintf!("expected error type Error, got %s", got));
        }
        let got = type_string(&mut tp, parsed.r_in);
        if got != "number" {
            t.fatal(sprintf!("expected input requirements type number, got %s", got));
        }
    });
}

// go test: internal/typeparser/location_independent_type_parser_test.go:69 TestEffectSchemaTypesParsesInstantiatedV3TypeWithoutLocation
#[test]
fn test_effect_schema_types_parses_instantiated_v3_type_without_location() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_package_installed(EFFECT_V3, "effect") {
            t.skip(sprintln!("Effect v3 not installed:", err));
        }

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_with_effect_v3_internal(
            t,
            r#"
import * as Schema from "effect/Schema"

declare const schema: Schema.Schema<string, number>
schema
"#,
        );
        let mut tp = compiled.type_parser();

        let schema_type = tp.get_type_at_location(find_identifier_by_text(t, sf, "schema", 1));
        let Some(parsed) = tp.effect_schema_types(schema_type) else {
            t.fatal("expected instantiated v3 Schema type to parse without a location");
        };
        let got = type_string(&mut tp, parsed.a);
        if got != "string" {
            t.fatal(sprintf!("expected decoded type string, got %s", got));
        }
        let got = type_string(&mut tp, parsed.e);
        if got != "number" {
            t.fatal(sprintf!("expected encoded type number, got %s", got));
        }
    });
}

// go test: internal/typeparser/location_independent_type_parser_test.go:96 TestStreamTypeParsesInstantiatedV3TypeWithoutLocation
#[test]
fn test_stream_type_parses_instantiated_v3_type_without_location() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_package_installed(EFFECT_V3, "effect") {
            t.skip(sprintln!("Effect v3 not installed:", err));
        }

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_with_effect_v3_internal(
            t,
            r#"
import * as Stream from "effect/Stream"

declare const stream: Stream.Stream<string, Error, number>
stream
"#,
        );
        let mut tp = compiled.type_parser();

        let stream_type = tp.get_type_at_location(find_identifier_by_text(t, sf, "stream", 1));
        let Some(parsed) = tp.stream_type(stream_type) else {
            t.fatal("expected instantiated v3 Stream type to parse without a location");
        };
        let got = type_string(&mut tp, parsed.a);
        if got != "string" {
            t.fatal(sprintf!("expected element type string, got %s", got));
        }
        let got = type_string(&mut tp, parsed.e);
        if got != "Error" {
            t.fatal(sprintf!("expected error type Error, got %s", got));
        }
        let got = type_string(&mut tp, parsed.r);
        if got != "number" {
            t.fatal(sprintf!("expected requirements type number, got %s", got));
        }
    });
}
