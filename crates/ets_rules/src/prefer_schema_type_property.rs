use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::DO_NOT_USE_SCHEMA_SCHEMA_TYPE_TYPEOF_X_TO_EXTRACT_A_SCHEMA_S_TYPE_USE_TYPEOF_X_TYPE_INSTEAD_EFFECT_PREFER_SCHEMA_TYPE_PROPERTY;

// go: internal/rules/prefer_schema_type_property.go:13 PreferSchemaTypeProperty
pub static PREFER_SCHEMA_TYPE_PROPERTY: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "preferSchemaTypeProperty",
    group: "style",
    description: "Disallows Schema.Schema.Type<typeof X> in favor of typeof X.Type",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_prefer_schema_type_property(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/prefer_schema_type_property.go:30 PreferSchemaTypePropertyMatch
#[derive(Clone)]
pub struct PreferSchemaTypePropertyMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub type_reference: P<Node>,
    pub schema_name: P<Node>,
    pub schema_name_text: String,
}

// go: internal/rules/prefer_schema_type_property.go:38 AnalyzePreferSchemaTypeProperty
pub fn analyze_prefer_schema_type_property(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<PreferSchemaTypePropertyMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        node: P<Node>,
        matches: &mut Vec<PreferSchemaTypePropertyMatch>,
    ) -> bool {
        if node.kind() == Kind::TypeReference
            && let Some(m) = analyze_prefer_schema_type_property_node(tp, sf, node)
        {
            matches.push(m);
            return false;
        }
        node.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}

// go: internal/rules/prefer_schema_type_property.go:58 analyzePreferSchemaTypePropertyNode
pub(crate) fn analyze_prefer_schema_type_property_node(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Option<PreferSchemaTypePropertyMatch> {
    let reference = node.as_type_reference_node();
    let type_arguments = reference.type_arguments()?;
    if type_arguments.nodes().len() != 1 {
        return None;
    }

    let type_name = reference.type_name();
    if type_name.kind() != Kind::QualifiedName {
        return None;
    }
    let outer_name = type_name.as_qualified_name();
    if outer_name.right().text() != "Type" {
        return None;
    }
    let middle_name = outer_name.left();
    if middle_name.kind() != Kind::QualifiedName {
        return None;
    }
    let type_argument = type_arguments.nodes()[0];
    if type_argument.kind() != Kind::TypeQuery {
        return None;
    }
    let query = type_argument.as_type_query_node();
    if query.type_arguments().is_some_and(|args| !args.nodes().is_empty()) {
        return None;
    }

    let schema_export = middle_name.as_qualified_name().right();
    if !tp.is_node_reference_to_effect_schema_module_api(schema_export, "Schema") {
        return None;
    }

    Some(PreferSchemaTypePropertyMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, node),
        type_reference: node,
        schema_name: query.expr_name(),
        schema_name_text: ets_shim::scanner::get_text_of_node(query.expr_name()),
    })
}
