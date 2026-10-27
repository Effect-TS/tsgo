// Ported from effect-tsgo's internal/typeparser/extends_schema_brand_test.go (see helpers_test.rs). Rust: the
// parsers return their cached result by value (a `Copy` struct), so Go's "cached result pointer" checks compare the
// repeated results for equality.

use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::core::P;

use crate::helpers_test::*;
use crate::module_export_reference_test::kind_string;
use crate::service_type_test::find_class_type_by_name;
use crate::*;

/// Rust-only: Go's `any` holding the parsers' typed results.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SchemaParseResult {
    Class(Option<SchemaClassResult>),
    Tagged(Option<SchemaTaggedResult>),
    Opaque(Option<SchemaOpaqueResult>),
}

// go test: internal/typeparser/extends_schema_brand_test.go:12 TestExtendsSchemaBrandSyntax 749b14a366630ea2
#[test]
fn test_extends_schema_brand_syntax() {
    run(|t| {
        t.parallel();
        struct Constructor {
            member: &'static str,
            inner_args: &'static str,
            outer_args: &'static str,
        }
        let constructors = [
            Constructor {
                member: "Class",
                inner_args: r#""schema-key""#,
                outer_args: "{ value: Schema.String }",
            },
            Constructor {
                member: "TaggedClass",
                inner_args: r#""schema-key""#,
                outer_args: r#""schema-tag", { value: Schema.String }"#,
            },
            Constructor {
                member: "Error",
                inner_args: r#""schema-key""#,
                outer_args: "{ value: Schema.String }",
            },
            Constructor {
                member: "TaggedError",
                inner_args: r#""schema-key""#,
                outer_args: r#""schema-tag", { value: Schema.String }"#,
            },
            Constructor {
                member: "Opaque",
                inner_args: "",
                outer_args: "Schema.Struct({ value: Schema.String })",
            },
        ];
        struct Brand {
            name: &'static str,
            text: &'static str,
            kind: Kind,
        }
        let brands = [
            Brand { name: "Unbranded", text: "", kind: Kind::Unknown },
            Brand { name: "Literal", text: r#""Brand""#, kind: Kind::LiteralType },
            Brand {
                name: "UniqueSymbol",
                text: "{ readonly brand: unique symbol }",
                kind: Kind::TypeLiteral,
            },
            Brand { name: "Alias", text: "BrandAlias", kind: Kind::TypeReference },
            Brand { name: "Empty", text: "{}", kind: Kind::TypeLiteral },
        ];
        let imports = [("Namespace", "Schema."), ("ModuleNamespace", "S."), ("Renamed", "Make")];
        let mut source = String::new();
        source.push_str(
            r#"
import { Schema } from "effect"
import * as S from "effect/Schema"
import { Class as MakeClass, TaggedClass as MakeTaggedClass, Error as MakeError,
  TaggedError as MakeTaggedError, Opaque as MakeOpaque } from "effect/Schema"
type BrandAlias = { readonly brand: unique symbol }
const schemaKey = "dynamic-key"
const schemaTag = "dynamic-tag"
"#,
        );
        source.push_str("namespace Local {\n");
        for constructor in &constructors {
            source.push_str(&sprintf!(
                "export const %s = <Self, Brand = {}>(...args: unknown[]) => (...fields: unknown[]) => class {}\n",
                constructor.member
            ));
        }
        source.push_str("}\n");
        struct TestCase {
            name: String,
            member: &'static str,
            brand: String,
            key: String,
            tag: String,
            brand_kind: Kind,
            r#match: bool,
        }
        let mut tests: Vec<TestCase> = Vec::new();
        for constructor in &constructors {
            let member = constructor.member;
            let mut add = |suffix: &str,
                           expression: &str,
                           brand: &str,
                           kind: Kind,
                           r#match: bool,
                           key: &str,
                           tag: &str| {
                let name = format!("{member}{suffix}");
                let expression = expression.replace("$Self", &name);
                source.push_str(&sprintf!("class %s extends %s {}\n", name, expression));
                tests.push(TestCase {
                    name,
                    member,
                    brand: brand.to_string(),
                    key: key.to_string(),
                    tag: tag.to_string(),
                    brand_kind: kind,
                    r#match,
                });
            };
            let (mut key, mut tag) = ("", "");
            if member == "TaggedClass" || member == "TaggedError" {
                (key, tag) = (r#""schema-key""#, r#""schema-tag""#);
            }
            for (imported_name, imported_prefix) in imports {
                for brand in &brands {
                    let mut type_args = "$Self".to_string();
                    if !brand.text.is_empty() {
                        type_args += ", ";
                        type_args += brand.text;
                    }
                    let expression = sprintf!(
                        "%s%s<%s>(%s)(%s)",
                        imported_prefix,
                        member,
                        type_args,
                        constructor.inner_args,
                        constructor.outer_args
                    );
                    add(
                        &format!("{imported_name}{}", brand.name),
                        &expression,
                        brand.text,
                        brand.kind,
                        true,
                        key,
                        tag,
                    );
                }
            }
            for (invalid_name, invalid_expression) in [
                (
                    "MissingSelf",
                    sprintf!(
                        "Schema.%s(%s)(%s)",
                        member,
                        constructor.inner_args,
                        constructor.outer_args
                    ),
                ),
                ("SingleCall", sprintf!("Schema.%s<$Self>(%s)", member, constructor.inner_args)),
                (
                    "Lookalike",
                    sprintf!(
                        "Local.%s<$Self, BrandAlias>(%s)(%s)",
                        member,
                        constructor.inner_args,
                        constructor.outer_args
                    ),
                ),
            ] {
                add(invalid_name, &invalid_expression, "", Kind::Unknown, false, "", "");
            }
            add(
                "ThreeArgs",
                &sprintf!(
                    "Schema.%s<$Self, BrandAlias, {}>(%s)(%s)",
                    member,
                    constructor.inner_args,
                    constructor.outer_args
                ),
                "BrandAlias",
                Kind::TypeReference,
                member != "Opaque",
                key,
                tag,
            );
            if !key.is_empty() {
                add(
                    "DynamicLiterals",
                    &sprintf!(
                        "Schema.%s<$Self, {}>(schemaKey)(schemaTag, { value: Schema.String })",
                        member
                    ),
                    "{}",
                    Kind::TypeLiteral,
                    true,
                    "",
                    "",
                );
                add(
                    "OmittedKey",
                    &sprintf!(
                        "Schema.%s<$Self>()(\"schema-tag\", { value: Schema.String })",
                        member
                    ),
                    "",
                    Kind::Unknown,
                    true,
                    "",
                    tag,
                );
            }
        }
        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, &source);
        let mut tp = compiled.type_parser();
        // Rust: Go's `parsers` map of closures, keyed by member.
        let parse = |tp: &mut TypeParser<'_>, member: &str, node: P<Node>| match member {
            "Class" => SchemaParseResult::Class(tp.extends_schema_class(node)),
            "TaggedClass" => SchemaParseResult::Tagged(tp.extends_schema_tagged_class(node)),
            "Error" => SchemaParseResult::Class(tp.extends_schema_error(node)),
            "TaggedError" => SchemaParseResult::Tagged(tp.extends_schema_tagged_error(node)),
            "Opaque" => SchemaParseResult::Opaque(tp.extends_schema_opaque(node)),
            _ => unreachable!("no parser for {member}"),
        };
        for test in &tests {
            let name = find_class_type_by_name(t, &mut tp, sf, &test.name).0;
            let Some(class) = name.parent() else {
                t.fatal("class name has no parent");
            };
            if test.member == "Class" && tp.extends_schema_error(class).is_some() {
                t.fatal("Class must not match Error");
            }
            if test.member == "Error" && tp.extends_schema_class(class).is_some() {
                t.fatal("Error must not match Class");
            }
            let got = parse(&mut tp, test.member, class);
            if parse(&mut tp, test.member, class) != got {
                t.fatal("repeated parse did not return the cached result pointer");
            }
            let (mut self_, mut brand, mut class_name, mut key, mut tag): (
                Option<P<Node>>,
                Option<P<Node>>,
                Option<P<Node>>,
                Option<P<Node>>,
                Option<P<Node>>,
            ) = (None, None, None, None, None);
            let matched;
            // Rust: Go's `default` case (an unexpected result type) cannot happen with a closed enum.
            match got {
                SchemaParseResult::Class(result) => {
                    matched = result.is_some();
                    if let Some(result) = result {
                        (self_, brand, class_name) = (
                            Some(result.self_type_node),
                            result.brand_type_node,
                            Some(result.class_name),
                        );
                    }
                }
                SchemaParseResult::Tagged(result) => {
                    matched = result.is_some();
                    if let Some(result) = result {
                        (self_, brand, class_name) = (
                            Some(result.self_type_node),
                            result.brand_type_node,
                            Some(result.class_name),
                        );
                        (key, tag) = (result.key_string_literal, result.tag_string_literal);
                    }
                }
                SchemaParseResult::Opaque(result) => {
                    matched = result.is_some();
                    if let Some(result) = result {
                        (self_, brand) = (Some(result.self_type_node), result.brand_type_node);
                    }
                }
            }
            if matched != test.r#match {
                t.fatal(sprintf!("matched = %v, want %v", matched, test.r#match));
            }
            if !test.r#match {
                continue;
            }
            assert_schema_extends_node(t, sf, self_, Kind::TypeReference, &test.name);
            assert_schema_extends_node(t, sf, brand, test.brand_kind, &test.brand);
            if test.member != "Opaque" {
                assert_schema_extends_node(t, sf, class_name, Kind::Identifier, &test.name);
                if class_name != Some(name) {
                    t.fatal("ClassName is not the original class identifier");
                }
            }
            assert_schema_extends_node(t, sf, key, Kind::StringLiteral, &test.key);
            assert_schema_extends_node(t, sf, tag, Kind::StringLiteral, &test.tag);
            let outer = ets_shim::ast::get_extends_heritage_clause_elements(class)[0]
                .as_expression_with_type_arguments()
                .expression()
                .as_call_expression();
            let inner = outer.expression().as_call_expression();
            let inner_type_arguments =
                inner.type_arguments().map(|l| l.nodes()).unwrap_or_default();
            if self_ != Some(inner_type_arguments[0]) {
                t.fatal("SelfTypeNode is not the original first type argument");
            }
            if !test.brand.is_empty() && brand != Some(inner_type_arguments[1]) {
                t.fatal("BrandTypeNode is not the original second type argument");
            }
            if key.is_some() && key != Some(inner.arguments().nodes()[0]) {
                t.fatal("KeyStringLiteral is not the original identifier argument");
            }
            if tag.is_some() && tag != Some(outer.arguments().nodes()[0]) {
                t.fatal("TagStringLiteral is not the original tag argument");
            }
        }
    });
}

// go test: internal/typeparser/extends_schema_brand_test.go:172 assertSchemaExtendsNode 79a5cc6b770c91a1
fn assert_schema_extends_node(
    t: &T,
    sf: P<SourceFile>,
    node: Option<P<Node>>,
    kind: Kind,
    text: &str,
) {
    t.helper();
    if text.is_empty() {
        if let Some(node) = node {
            t.fatal(sprintf!("expected nil node, got kind %v", kind_string(node.kind())));
        }
        return;
    }
    let Some(node) = node else {
        t.fatal(sprintf!("expected %q node, got nil", text));
    };
    if node.kind() != kind {
        t.fatal(sprintf!(
            "%q node kind = %v, want %v",
            text,
            kind_string(node.kind()),
            kind_string(kind)
        ));
    }
    let got = ets_shim::scanner::get_source_text_of_node_from_source_file(sf, node, false);
    if got != text {
        t.fatal(sprintf!("node text = %q, want %q", got, text));
    }
}

// go test: internal/typeparser/extends_schema_brand_test.go:191 TestExtendsSchemaRequestsHaveNoBrand e5a02fd4cf35391c
#[test]
fn test_extends_schema_requests_have_no_brand() {
    run(|t| {
        t.parallel();
        let source = r#"
import { Schema } from "effect"
import { Class } from "effect/Schema"
declare module "effect/Schema" { export { Class as RequestClass } }
class AliasedRequestClass extends Schema.RequestClass<AliasedRequestClass>("request-key")({ value: Schema.String }) {}
class AliasedRequestClassSecondGeneric extends Schema.RequestClass<AliasedRequestClassSecondGeneric, "Brand">("request-key")({ value: Schema.String }) {}
class Unbranded extends Schema.TaggedRequest<Unbranded>("request-key")("request-tag", {
  payload: {}, success: Schema.Void, failure: Schema.Never
}) {}
class SecondGeneric extends Schema.TaggedRequest<SecondGeneric, "Brand">("request-key")("request-tag", {
  payload: {}, success: Schema.Void, failure: Schema.Never
}) {}
"#;
        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v3_internal(t, source);
        let mut tp = compiled.type_parser();
        for name in ["AliasedRequestClass", "AliasedRequestClassSecondGeneric"] {
            let class_name = find_class_type_by_name(t, &mut tp, sf, name).0;
            let Some(class) = class_name.parent() else {
                t.fatal("class name has no parent");
            };
            let Some(got) = tp.extends_schema_request_class(class) else {
                t.fatal("expected RequestClass match");
            };
            assert_schema_extends_node(t, sf, Some(got.class_name), Kind::Identifier, name);
            assert_schema_extends_node(t, sf, Some(got.self_type_node), Kind::TypeReference, name);
            assert_schema_extends_node(t, sf, got.brand_type_node, Kind::Unknown, "");
            if tp.extends_schema_request_class(class) != Some(got) {
                t.fatal("RequestClass did not return its cached result pointer");
            }
        }
        for name in ["Unbranded", "SecondGeneric"] {
            let class_name = find_class_type_by_name(t, &mut tp, sf, name).0;
            let Some(class) = class_name.parent() else {
                t.fatal("class name has no parent");
            };
            let Some(got) = tp.extends_schema_tagged_request(class) else {
                t.fatal("expected TaggedRequest match");
            };
            assert_schema_extends_node(t, sf, Some(got.class_name), Kind::Identifier, name);
            assert_schema_extends_node(t, sf, Some(got.self_type_node), Kind::TypeReference, name);
            assert_schema_extends_node(t, sf, got.brand_type_node, Kind::Unknown, "");
            assert_schema_extends_node(
                t,
                sf,
                got.key_string_literal,
                Kind::StringLiteral,
                r#""request-key""#,
            );
            assert_schema_extends_node(
                t,
                sf,
                got.tag_string_literal,
                Kind::StringLiteral,
                r#""request-tag""#,
            );
            if tp.extends_schema_tagged_request(class) != Some(got) {
                t.fatal("TaggedRequest did not return its cached result pointer");
            }
            if tp.extends_schema_tagged_class(class).is_some()
                || tp.extends_schema_tagged_error(class).is_some()
                || tp.extends_schema_request_class(class).is_some()
            {
                t.fatal("TaggedRequest matched a different Schema member");
            }
        }
    });
}
