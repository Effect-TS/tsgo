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
    &tsdiag::PREFER_SCHEMA_TAGGED_ERROR_OVER_DATA_TAGGED_ERROR_FOR_AN_EXPLICIT_SCHEMA_CONTRACT_EFFECT_PREFER_SCHEMA_TAGGED_ERROR;

// go: internal/rules/prefer_schema_tagged_error.go:13 PreferSchemaTaggedError d124bb5317dd9c2a
pub static PREFER_SCHEMA_TAGGED_ERROR: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "preferSchemaTaggedError",
    group: "style",
    description: "Suggests using Schema.TaggedError instead of Data.TaggedError",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_prefer_schema_tagged_error(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/prefer_schema_tagged_error.go:30 PreferSchemaTaggedErrorMatch bf6d6d479a6994f3
#[derive(Clone)]
pub struct PreferSchemaTaggedErrorMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
}

// go: internal/rules/prefer_schema_tagged_error.go:35 AnalyzePreferSchemaTaggedError 5ac25019dbcf2321
pub fn analyze_prefer_schema_tagged_error(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<PreferSchemaTaggedErrorMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        node: P<Node>,
        matches: &mut Vec<PreferSchemaTaggedErrorMatch>,
    ) -> bool {
        if node.kind() == Kind::ClassDeclaration
            && let Some(result) = tp.extends_data_tagged_error(node)
        {
            let location_node = result.class_name().unwrap_or_else(|| result.call_expression());
            matches.push(PreferSchemaTaggedErrorMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, location_node),
            });
        }
        node.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}
