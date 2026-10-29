use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::checker::{CheckMode, Checker, ObjectFlags, Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    EffectMajorVersion, PipingFlow, PipingFlowTransformation, TransformationKind, TypeParser,
};
use rustc_hash::FxHashSet;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::THIS_INPUT_IS_ALREADY_ASSIGNABLE_TO_THE_SCHEMA_S_ENCODED_TYPE_USE_0_TO_PRESERVE_COMPILE_TIME_TYPE_CHECKING_INSTEAD_OF_DISCARDING_THE_INPUT_TYPE_THROUGH_1_EFFECT_PREFER_TYPED_SCHEMA_DECODER;

// go: internal/rules/prefer_typed_schema_decoder.go:16 typedSchemaDecoders e9e04dd38f8b2368
//
// Rust: Go's map is only iterated to find the one entry whose unknown-input name the callee references; a callee
// references at most one of these names, so the iteration order is not observable and a slice in source order
// stands in for it.
pub(crate) static TYPED_SCHEMA_DECODERS: &[(&str, &str)] = &[
    ("decodeUnknownEffect", "decodeEffect"),
    ("decodeUnknownSync", "decodeSync"),
    ("decodeUnknownExit", "decodeExit"),
    ("decodeUnknownOption", "decodeOption"),
    ("decodeUnknownResult", "decodeResult"),
    ("decodeUnknownPromise", "decodePromise"),
];

// go: internal/rules/prefer_typed_schema_decoder.go:27 PreferTypedSchemaDecoder 6ec82ea3a77de9f9
// PreferTypedSchemaDecoder suggests typed Schema decoders when the input is
// statically assignable to the schema's Encoded type.
pub static PREFER_TYPED_SCHEMA_DECODER: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "preferTypedSchemaDecoder",
    group: "style",
    description: "Suggests typed Schema decoders when the input is assignable to the schema's Encoded type",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_prefer_typed_schema_decoder(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.typed_name, m.unknown_name],
            ));
        }
        diags
    },
});

// go: internal/rules/prefer_typed_schema_decoder.go:53 PreferTypedSchemaDecoderMatch 0d0d5294e45edcfe
#[derive(Clone)]
pub struct PreferTypedSchemaDecoderMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub decoder_name: P<Node>,
    pub unknown_name: &'static str,
    pub typed_name: &'static str,
}

// go: internal/rules/prefer_typed_schema_decoder.go:63 AnalyzePreferTypedSchemaDecoder ca05f1f33763b42e
// AnalyzePreferTypedSchemaDecoder finds unknown-input decoder applications whose
// input is statically accepted by the schema's Encoded type.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub fn analyze_prefer_typed_schema_decoder(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<PreferTypedSchemaDecoderMatch> {
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, false);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        for (i, transformation) in flow.transformations().all() {
            let (Some(callee), Some(schema)) = schema_decoder_transformation(transformation) else {
                continue;
            };
            let input_node = PipingFlow::transformation_input_node(Some(flow), i);
            let input_type = PipingFlow::transformation_input_type(Some(flow), tp, i);
            if let Some(m) = analyze_typed_schema_decoder_application(
                tp, sf, callee, schema, input_node, input_type,
            ) {
                matches.push(m);
            }
        }
    }
    ets_go::sort::slice(&mut matches, |a, b| a.location.pos() < b.location.pos());

    matches
}

// go: internal/rules/prefer_typed_schema_decoder.go:90 schemaDecoderTransformation 4c8a04f33a8e8764
// schemaDecoderTransformation returns the decoder callee and the schema it was
// built from: the first pipeable argument, or the first argument of a decoder
// factory applied directly as in Schema.decodeUnknownSync(Person)(input).
//
// Rust: a transformation's callee and a call's expression and arguments are never nil.
pub(crate) fn schema_decoder_transformation(
    transformation: &PipingFlowTransformation,
) -> (Option<P<Node>>, Option<P<Node>>) {
    match transformation.kind() {
        TransformationKind::Pipe
        | TransformationKind::Pipeable
        | TransformationKind::DataFirst
        | TransformationKind::DataLast => {
            if transformation.args().len() > 0 {
                return (Some(transformation.callee()), Some(*transformation.args().at(0)));
            }
        }
        TransformationKind::Call | TransformationKind::CallWithOptionals => {
            // Decomposed curried pipeables expose their factory arguments as args.
            if transformation.kind() == TransformationKind::Call && transformation.args().len() > 0
            {
                return (Some(transformation.callee()), Some(*transformation.args().at(0)));
            }
            if transformation.callee().kind() == Kind::CallExpression {
                let factory = transformation.callee().as_call_expression();
                let factory_arguments = factory.arguments().nodes();
                if !factory_arguments.is_empty() {
                    return (Some(factory.expression()), Some(factory_arguments[0]));
                }
            }
        }
        _ => {}
    }
    (None, None)
}

// go: internal/rules/prefer_typed_schema_decoder.go:115 analyzeTypedSchemaDecoderApplication c780b0564a57a3d1
pub(crate) fn analyze_typed_schema_decoder_application(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    callee: P<Node>,
    schema: P<Node>,
    input_node: Option<P<Node>>,
    input_type: Option<P<Type>>,
) -> Option<PreferTypedSchemaDecoderMatch> {
    let name_node = schema_decoder_name_node(Some(callee))?;
    if tp.node_cannot_be_effect_library_type(schema) {
        return None;
    }
    let (unknown_name, typed_name) = match_unknown_schema_decoder(tp, callee);
    if unknown_name.is_empty() {
        return None;
    }

    let schema_at_location = tp.get_type_at_location(schema);
    let schema_type = tp.effect_schema_types(schema_at_location)?;
    let encoded = schema_type.e();
    let mut input_type = input_type;
    if input_type.is_none()
        && let Some(input_node) = input_node
    {
        input_type = tp.get_type_at_location(input_node);
    }
    let input_type = input_type?;
    if input_type.flags().intersects(TypeFlags::Any | TypeFlags::Unknown)
        || contains_unresolved_type_variable(
            tp.checker(),
            Some(input_type),
            &mut FxHashSet::default(),
        )
    {
        return None;
    }

    let mut assignable_type = input_type;
    if let Some(input_node) = input_node {
        let literal = ast::skip_parentheses(input_node);
        if literal.kind() == Kind::ObjectLiteralExpression
            || literal.kind() == Kind::ArrayLiteralExpression
        {
            assignable_type = tp.checker().check_expression_with_contextual_type(
                literal,
                encoded,
                None,
                CheckMode::TypeOnly,
            );
        }
    }
    if !tp.checker().is_type_assignable_to(assignable_type, encoded) {
        return None;
    }

    Some(PreferTypedSchemaDecoderMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, name_node),
        decoder_name: name_node,
        unknown_name,
        typed_name,
    })
}

// go: internal/rules/prefer_typed_schema_decoder.go:158 containsUnresolvedTypeVariable 45165762788695ff
pub(crate) fn contains_unresolved_type_variable(
    c: &mut Checker,
    t: Option<P<Type>>,
    seen: &mut FxHashSet<P<Type>>,
) -> bool {
    let Some(t) = t else {
        return false;
    };
    if seen.contains(&t) {
        return false;
    }
    seen.insert(t);
    if t.flags().intersects(TypeFlags::TypeVariable) {
        return true;
    }
    if t.flags().intersects(TypeFlags::UnionOrIntersection) {
        for &part in t.types() {
            if contains_unresolved_type_variable(c, Some(part), seen) {
                return true;
            }
        }
    }
    if t.object_flags().intersects(ObjectFlags::Reference) {
        for &arg in c.get_type_arguments(t) {
            if contains_unresolved_type_variable(c, Some(arg), seen) {
                return true;
            }
        }
    }
    for &property in c.get_properties_of_type_exported(t) {
        let property_type = c.get_type_of_symbol_exported(property);
        if contains_unresolved_type_variable(c, Some(property_type), seen) {
            return true;
        }
    }
    for &index_info in c.get_index_infos_of_type(t) {
        if contains_unresolved_type_variable(c, index_info.value_type.get(), seen) {
            return true;
        }
    }
    false
}

// go: internal/rules/prefer_typed_schema_decoder.go:193 matchUnknownSchemaDecoder 5fbde6c52cc7a4b5
pub(crate) fn match_unknown_schema_decoder(
    tp: &mut TypeParser<'_>,
    callee: P<Node>,
) -> (&'static str, &'static str) {
    for &(unknown_name, typed_name) in TYPED_SCHEMA_DECODERS {
        if tp.is_node_reference_to_effect_schema_module_api(callee, unknown_name)
            || tp.is_node_reference_to_effect_schema_parser_module_api(callee, unknown_name)
        {
            return (unknown_name, typed_name);
        }
    }
    ("", "")
}

// go: internal/rules/prefer_typed_schema_decoder.go:203 schemaDecoderNameNode 683e71467efadd80
pub(crate) fn schema_decoder_name_node(callee: Option<P<Node>>) -> Option<P<Node>> {
    let callee = callee?;
    if callee.kind() == Kind::PropertyAccessExpression {
        return Some(callee.as_property_access_expression().name());
    }
    if callee.kind() == Kind::Identifier {
        return Some(callee);
    }
    None
}
