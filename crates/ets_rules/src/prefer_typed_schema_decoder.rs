use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::checker::{CheckMode, Checker, ObjectFlags, Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectMajorVersion, PipingFlow, PipingFlowTransformation, TypeParser};
use rustc_hash::FxHashSet;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::This_input_is_already_assignable_to_the_schema_s_Encoded_type_Use_0_to_preserve_compile_time_type_checking_instead_of_discarding_the_input_type_through_1_effect_preferTypedSchemaDecoder;

// go: internal/rules/prefer_typed_schema_decoder.go:16 typedSchemaDecoders
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

// go: internal/rules/prefer_typed_schema_decoder.go:27 PreferTypedSchemaDecoder
// PreferTypedSchemaDecoder suggests typed Schema decoders when the input is
// statically assignable to the schema's Encoded type.
pub static PREFER_TYPED_SCHEMA_DECODER: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "preferTypedSchemaDecoder",
    group: "style",
    description: "Suggests typed Schema decoders when the input is assignable to the schema's Encoded type",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
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

// go: internal/rules/prefer_typed_schema_decoder.go:53 PreferTypedSchemaDecoderMatch
#[derive(Clone)]
pub struct PreferTypedSchemaDecoderMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub decoder_name: P<Node>,
    pub unknown_name: &'static str,
    pub typed_name: &'static str,
}

// go: internal/rules/prefer_typed_schema_decoder.go:63 AnalyzePreferTypedSchemaDecoder
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
    let mut handled_decoder_names: FxHashSet<P<Node>> = FxHashSet::default();

    let flows = tp.piping_flows(sf, false);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        for (i, transformation) in flow.transformations().all() {
            let (Some(callee), Some(schema)) = schema_decoder_transformation(transformation) else {
                continue;
            };
            let mut input_node = None;
            if i == 0 {
                input_node = flow.subject().node();
            }
            let input_type = PipingFlow::transformation_input_type(Some(flow), tp, i);
            if let Some(m) = analyze_typed_schema_decoder_application(
                tp, sf, callee, schema, input_node, input_type,
            ) {
                handled_decoder_names.insert(m.decoder_name);
                matches.push(m);
            }
        }
    }

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        node: P<Node>,
        matches: &mut Vec<PreferTypedSchemaDecoderMatch>,
        handled_decoder_names: &mut FxHashSet<P<Node>>,
    ) -> bool {
        if node.kind() == Kind::CallExpression {
            let call = node.as_call_expression();
            let arguments = call.arguments().nodes();
            if !arguments.is_empty() && call.expression().kind() == Kind::CallExpression {
                let factory = call.expression().as_call_expression();
                let factory_arguments = factory.arguments().nodes();
                if !factory_arguments.is_empty()
                    && let Some(m) = analyze_typed_schema_decoder_application(
                        tp,
                        sf,
                        factory.expression(),
                        factory_arguments[0],
                        Some(arguments[0]),
                        None,
                    )
                    && !handled_decoder_names.contains(&m.decoder_name)
                {
                    handled_decoder_names.insert(m.decoder_name);
                    matches.push(m);
                }
            }
        }
        node.for_each_child(&mut |child| walk(tp, sf, child, matches, handled_decoder_names));
        false
    }
    walk(tp, sf, sf.as_node(), &mut matches, &mut handled_decoder_names);
    // Rust: Go's `sort.Slice` is unstable; the keys are unique (each decoder name node is matched once, and two
    // name nodes never start at the same position), so a stable sort gives the same order.
    matches.sort_by_key(|m| m.location.pos());

    matches
}

// go: internal/rules/prefer_typed_schema_decoder.go:118 schemaDecoderTransformation
pub(crate) fn schema_decoder_transformation(
    transformation: &PipingFlowTransformation,
) -> (Option<P<Node>>, Option<P<Node>>) {
    if transformation.args().len() == 0 {
        return (None, None);
    }
    (Some(transformation.callee()), Some(*transformation.args().at(0)))
}

// go: internal/rules/prefer_typed_schema_decoder.go:125 analyzeTypedSchemaDecoderApplication
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

// go: internal/rules/prefer_typed_schema_decoder.go:168 containsUnresolvedTypeVariable
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

// go: internal/rules/prefer_typed_schema_decoder.go:203 matchUnknownSchemaDecoder
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

// go: internal/rules/prefer_typed_schema_decoder.go:213 schemaDecoderNameNode
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
