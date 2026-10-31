use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;
use rustc_hash::FxHashMap;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::DO_NOT_USE_0_TYPEOF_X_USE_TYPEOF_X_1_INSTEAD_EFFECT_PREFER_SCHEMA_TYPE_PROPERTY;

// go: internal/rules/prefer_schema_type_property.go:15 PreferSchemaTypeProperty ac0531a0a86fa4fe
pub static PREFER_SCHEMA_TYPE_PROPERTY: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "preferSchemaTypeProperty",
    group: "style",
    description: "Disallows Schema.Schema.Type<typeof X> and Schema.Codec.Encoded<typeof X>-style helpers in favor of typeof X.Type and typeof X.Encoded",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_prefer_schema_type_property(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[&m.helper_text, m.property_name],
            ));
        }
        diags
    },
});

// go: internal/rules/prefer_schema_type_property.go:32 PreferSchemaTypePropertyMatch 53e3ee9be2179cb5
#[derive(Clone)]
pub struct PreferSchemaTypePropertyMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub type_reference: P<Node>,
    pub schema_name: P<Node>,
    pub schema_name_text: String,
    pub helper_text: String,
    pub property_name: &'static str,
}

// go: internal/rules/prefer_schema_type_property.go:44 schemaPropertyHelperNamespaces 1b4d5712dc4b0d0c
pub(crate) static SCHEMA_PROPERTY_HELPER_NAMESPACES: LazyLock<
    FxHashMap<&'static str, &'static [&'static str]>,
> = LazyLock::new(|| {
    FxHashMap::from_iter([
        ("Type", &["Schema"][..]),
        ("Encoded", &["Schema", "Codec"][..]),
        ("Context", &["Schema"][..]),
        ("DecodingServices", &["Codec"][..]),
        ("EncodingServices", &["Codec"][..]),
    ])
});

// go: internal/rules/prefer_schema_type_property.go:52 AnalyzePreferSchemaTypeProperty e7c41852f6919645
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

// go: internal/rules/prefer_schema_type_property.go:73 analyzePreferSchemaTypePropertyNode 1a8eefa661bba717
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
    let property_name = outer_name.right().text();
    let (&property_name, &namespaces) =
        SCHEMA_PROPERTY_HELPER_NAMESPACES.get_key_value(property_name)?;
    let mut namespace_name = outer_name.left();
    if namespace_name.kind() == Kind::QualifiedName {
        namespace_name = namespace_name.as_qualified_name().right();
    }
    if namespace_name.kind() != Kind::Identifier {
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

    if !namespaces.iter().any(|namespace| {
        tp.is_node_reference_to_effect_schema_module_api(namespace_name, namespace)
    }) {
        return None;
    }

    Some(PreferSchemaTypePropertyMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, node),
        type_reference: node,
        schema_name: query.expr_name(),
        schema_name_text: ets_shim::scanner::get_text_of_node(query.expr_name()),
        helper_text: ets_shim::scanner::get_text_of_node(type_name),
        property_name,
    })
}
