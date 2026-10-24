use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectMajorVersion, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::The_payload_field_name_shadows_this_error_s_own_name_so_logs_and_String_error_show_the_field_value_instead_of_the_error_type_Consider_renaming_it_effect_errorPayloadShadowsName;

// go: internal/rules/error_payload_shadows_name.go:13 ErrorPayloadShadowsName
pub static ERROR_PAYLOAD_SHADOWS_NAME: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "errorPayloadShadowsName",
    group: "correctness",
    description: "Detects error payload fields named `name` that shadow the error's own name",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_error_payload_shadows_name(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/error_payload_shadows_name.go:30 ErrorPayloadShadowsNameMatch
#[derive(Clone)]
pub struct ErrorPayloadShadowsNameMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
}

// go: internal/rules/error_payload_shadows_name.go:35 AnalyzeErrorPayloadShadowsName
pub fn analyze_error_payload_shadows_name(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<ErrorPayloadShadowsNameMatch> {
    let mut matches = Vec::new();
    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        node: P<Node>,
        matches: &mut Vec<ErrorPayloadShadowsNameMatch>,
    ) -> bool {
        if node.kind() == Kind::ClassDeclaration || node.kind() == Kind::ClassExpression {
            let (fields, error_name) = error_payload_fields(tp, node);
            let mut members: &[P<Node>] = &[];
            if let Some(fields) = fields {
                match fields.kind() {
                    Kind::TypeLiteral => members = fields.members(),
                    Kind::ObjectLiteralExpression => members = fields.properties(),
                    _ => {}
                }
            }
            for &member in members {
                match member.kind() {
                    Kind::PropertySignature
                    | Kind::PropertyAssignment
                    | Kind::ShorthandPropertyAssignment => {
                        let Some(name) = member.name() else {
                            continue;
                        };
                        if ast::get_text_of_property_name(name) != "name"
                            || error_payload_name_matches_error_name(tp, member, error_name)
                        {
                            continue;
                        }
                        matches.push(ErrorPayloadShadowsNameMatch {
                            source_file: sf,
                            location: ets_shim::scanner::get_error_range_for_node(sf, name),
                        });
                    }
                    _ => {}
                }
            }
        }
        node.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }
    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}

// go: internal/rules/error_payload_shadows_name.go:71 errorPayloadFields
// Rust: Go's `(fields, errorName)` named results.
pub(crate) fn error_payload_fields(
    tp: &mut TypeParser<'_>,
    class_node: P<Node>,
) -> (Option<P<Node>>, Option<P<Node>>) {
    if let Some(result) = tp.extends_data_tagged_error(class_node) {
        return (result.fields_type_node(), result.key_string_literal());
    }
    if let Some(result) = tp.extends_data_error(class_node) {
        return (result.fields_type_node(), None);
    }
    if let Some(result) = tp.extends_schema_tagged_error(class_node) {
        if tp.supported_effect_version() == EffectMajorVersion::V4
            && result.key_string_literal().is_some()
        {
            return (result.fields_node(), result.key_string_literal());
        }
        return (result.fields_node(), result.tag_string_literal());
    }
    if let Some(result) = tp.extends_schema_error(class_node) {
        return (result.fields_node(), result.key_string_literal());
    }
    (None, None)
}

// go: internal/rules/error_payload_shadows_name.go:90 errorPayloadNameMatchesErrorName
pub(crate) fn error_payload_name_matches_error_name(
    tp: &mut TypeParser<'_>,
    member: P<Node>,
    error_name: Option<P<Node>>,
) -> bool {
    let Some(error_name) = error_name else {
        return false;
    };
    if !ast::is_string_literal(error_name) {
        return false;
    }
    let mut literal = None;
    match member.kind() {
        Kind::PropertySignature => {
            let type_node = member.as_property_signature_declaration().type_();
            if let Some(type_node) = type_node
                && type_node.kind() == Kind::LiteralType
            {
                literal = Some(type_node.as_literal_type_node().literal());
            }
        }
        Kind::PropertyAssignment => {
            let initializer = member.initializer();
            if let Some(initializer) = initializer
                && ast::is_call_expression(initializer)
            {
                let call = initializer.as_call_expression();
                let arguments = call.arguments().nodes();
                if arguments.len() == 1
                    && (tp.is_node_reference_to_effect_schema_module_api(
                        call.expression(),
                        "Literal",
                    ) || tp
                        .is_node_reference_to_effect_schema_module_api(call.expression(), "tag")
                        || tp.is_node_reference_to_effect_schema_module_api(
                            call.expression(),
                            "tagDefaultOmit",
                        ))
                {
                    literal = Some(arguments[0]);
                }
            }
        }
        _ => {}
    }
    literal.is_some_and(|literal| {
        ast::is_string_literal(literal) && literal.text() == error_name.text()
    })
}
