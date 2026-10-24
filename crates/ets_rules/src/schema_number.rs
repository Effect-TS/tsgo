use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_readonly::Slice;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectMajorVersion, PipingFlow, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::This_Schema_number_API_accepts_NaN_Infinity_and_Infinity_Use_0_for_finite_domain_numbers_If_non_finite_values_are_intentional_disable_this_diagnostic_for_that_line_effect_schemaNumber;

// go: internal/rules/schema_number.go:14 SchemaNumber
pub static SCHEMA_NUMBER: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "schemaNumber",
    group: "style",
    description: "Suggests Schema.Finite and Schema.FiniteFromString instead of Schema.Number APIs when describing domain numbers",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_schema_number(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.replacement],
            ));
        }
        diags
    },
});

// go: internal/rules/schema_number.go:31 SchemaNumberMatch
#[derive(Clone)]
pub struct SchemaNumberMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub reference_node: P<Node>,
    pub replacement: &'static str,
    pub replacement_identifier: &'static str,
}

// go: internal/rules/schema_number.go:39 AnalyzeSchemaNumber
pub fn analyze_schema_number(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> Vec<SchemaNumberMatch> {
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }

    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        node: P<Node>,
        matches: &mut Vec<SchemaNumberMatch>,
    ) -> bool {
        match node.kind() {
            Kind::ImportDeclaration | Kind::ImportEqualsDeclaration => return false,
            Kind::PropertyAccessExpression => {
                if let Some(m) = analyze_schema_number_reference(tp, sf, node) {
                    matches.push(m);
                    return false;
                }
            }
            Kind::Identifier => {
                if let Some(m) = analyze_schema_number_reference(tp, sf, node) {
                    matches.push(m);
                }
            }
            _ => {}
        }

        node.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);

    let flows = tp.piping_flows(sf, false);
    let flows = ets_readonly::new(flows.as_deref());
    let mut filtered = Vec::with_capacity(matches.len());
    for m in matches {
        if schema_number_has_finite_method_check(tp, m.reference_node)
            || schema_number_has_finite_piping_check(tp, m.reference_node, flows)
        {
            continue;
        }
        filtered.push(m);
    }
    filtered
}

// go: internal/rules/schema_number.go:82 analyzeSchemaNumberReference
pub(crate) fn analyze_schema_number_reference(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Option<SchemaNumberMatch> {
    for api in SCHEMA_NUMBER_APIS {
        if tp.is_node_reference_to_effect_schema_module_api(node, api.name) {
            let reference_node = schema_number_reference_location(node);
            return Some(SchemaNumberMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, reference_node),
                reference_node,
                replacement: api.replacement,
                replacement_identifier: api.replacement_identifier,
            });
        }
    }
    None
}

// go: internal/rules/schema_number.go:98 schemaNumberHasFiniteMethodCheck
pub(crate) fn schema_number_has_finite_method_check(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
) -> bool {
    let mut schema_expression = node;
    if node.kind() == Kind::Identifier
        && let Some(parent) = node.parent()
        && parent.kind() == Kind::PropertyAccessExpression
    {
        let access = parent.as_property_access_expression();
        if access.name() == node {
            schema_expression = parent;
        }
    }
    while let Some(access_node) = schema_expression.parent()
        && access_node.kind() == Kind::PropertyAccessExpression
    {
        let access = access_node.as_property_access_expression();
        let Some(call_node) = access_node.parent() else {
            return false;
        };
        if access.expression() != schema_expression || call_node.kind() != Kind::CallExpression {
            return false;
        }

        let call = call_node.as_call_expression();
        if call.expression() != access_node {
            return false;
        }
        match access.name().text() {
            "annotate" => schema_expression = call_node,
            "check" => {
                return schema_number_has_finite_predicate(
                    tp,
                    ets_readonly::new(call.arguments().nodes()),
                );
            }
            _ => return false,
        }
    }
    false
}

// go: internal/rules/schema_number.go:132 schemaNumberHasFinitePipingCheck
pub(crate) fn schema_number_has_finite_piping_check(
    tp: &mut TypeParser<'_>,
    reference: P<Node>,
    flows: Slice<'_, PipingFlow>,
) -> bool {
    for (_, flow) in flows.all() {
        let Some(subject_node) = flow.subject().node() else {
            continue;
        };
        if !schema_number_node_contains(Some(subject_node), Some(reference)) {
            continue;
        }
        for (_, transformation) in flow.transformations().all() {
            if tp.is_node_reference_to_effect_schema_module_api(transformation.callee(), "check")
                && schema_number_has_finite_predicate(tp, transformation.args())
            {
                return true;
            }
        }
    }
    false
}

// go: internal/rules/schema_number.go:148 schemaNumberHasFinitePredicate
pub(crate) fn schema_number_has_finite_predicate(
    tp: &mut TypeParser<'_>,
    arguments: Slice<'_, P<Node>>,
) -> bool {
    for (_, &argument) in arguments.all() {
        let argument = ets_shim::ast::skip_parentheses(argument);
        if argument.kind() != Kind::CallExpression {
            continue;
        }
        let predicate_call = argument.as_call_expression();
        let expression = predicate_call.expression();
        if tp.is_node_reference_to_effect_schema_module_api(expression, "isFinite")
            || tp.is_node_reference_to_effect_schema_module_api(expression, "isInt")
        {
            return true;
        }
    }
    false
}

// go: internal/rules/schema_number.go:166 schemaNumberNodeContains
pub(crate) fn schema_number_node_contains(
    ancestor: Option<P<Node>>,
    target: Option<P<Node>>,
) -> bool {
    match (ancestor, target) {
        (Some(ancestor), Some(target)) => {
            ancestor.pos() <= target.pos() && target.end() <= ancestor.end()
        }
        _ => false,
    }
}

// go: internal/rules/schema_number.go:170 schemaNumberApi
pub(crate) struct SchemaNumberApi {
    pub(crate) name: &'static str,
    pub(crate) replacement: &'static str,
    pub(crate) replacement_identifier: &'static str,
}

// go: internal/rules/schema_number.go:176 schemaNumberApis
pub(crate) static SCHEMA_NUMBER_APIS: &[SchemaNumberApi] = &[
    SchemaNumberApi {
        name: "Number",
        replacement: "Schema.Finite",
        replacement_identifier: "Finite",
    },
    SchemaNumberApi {
        name: "NumberFromString",
        replacement: "Schema.FiniteFromString",
        replacement_identifier: "FiniteFromString",
    },
];

// go: internal/rules/schema_number.go:181 schemaNumberReferenceLocation
pub(crate) fn schema_number_reference_location(node: P<Node>) -> P<Node> {
    if node.kind() == Kind::PropertyAccessExpression {
        return node.as_property_access_expression().name();
    }
    node
}
