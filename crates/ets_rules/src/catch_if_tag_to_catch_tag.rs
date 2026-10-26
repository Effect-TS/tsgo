use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    LazyExpressionFlags, PipingFlowTransformation, TypeParser, parse_lazy_expression,
};

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_CATCH_TAG_EXPRESSES_TAGGED_ERROR_RECOVERY_MORE_DIRECTLY_THAN_EFFECT_CATCH_IF_WITH_A_TAG_EQUALITY_PREDICATE_EFFECT_CATCH_IF_TAG_TO_CATCH_TAG;

// go: internal/rules/catch_if_tag_to_catch_tag.go:14 CatchIfTagToCatchTag
pub static CATCH_IF_TAG_TO_CATCH_TAG: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "catchIfTagToCatchTag",
    group: "style",
    description: "Suggests Effect.catchTag instead of Effect.catchIf with a _tag equality or Predicate.isTagged predicate",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_catch_if_tag_to_catch_tag(ctx.type_parser, Some(source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diagnostics
    },
});

// go: internal/rules/catch_if_tag_to_catch_tag.go:33 CatchIfTagToCatchTagMatch
#[derive(Clone)]
pub struct CatchIfTagToCatchTagMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub transformation: PipingFlowTransformation,
    pub tag: String,
    pub handler: P<Node>,
    pub can_fix: bool,
}

// go: internal/rules/catch_if_tag_to_catch_tag.go:42 AnalyzeCatchIfTagToCatchTag
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`), never nil.
pub fn analyze_catch_if_tag_to_catch_tag(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<CatchIfTagToCatchTagMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        for (index, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(index).clone();
            if transformation.args().len() != 2
                || !tp.is_node_reference_to_effect_module_api(transformation.callee(), "catchIf")
            {
                continue;
            }
            let Some(tag) = catch_if_tag_predicate(tp, *transformation.args().at(0)) else {
                continue;
            };
            matches.push(CatchIfTagToCatchTagMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, transformation.callee()),
                handler: *transformation.args().at(1),
                transformation,
                tag,
                can_fix: true,
            });
        }
    }
    matches
}

// go: internal/rules/catch_if_tag_to_catch_tag.go:69 catchIfTagPredicate
pub(crate) fn catch_if_tag_predicate(
    tp: &mut TypeParser<'_>,
    predicate_node: P<Node>,
) -> Option<String> {
    let lazy = parse_lazy_expression(predicate_node, LazyExpressionFlags::None)?;
    if lazy.params().len() != 1 {
        return None;
    }
    let parameter = *lazy.params().at(0);
    let parameter_name = parameter.name()?;
    if parameter_name.kind() != Kind::Identifier {
        return None;
    }
    let parameter_symbol = tp.get_symbol_at_location(parameter_name)?;
    if tp.checker().is_symbol_assigned(parameter_symbol) {
        return None;
    }

    let (tag_subject, tag_value) = tp.parse_tag_match(lazy.expression());
    let tag_value = tag_value.filter(|v| ast::is_string_literal(*v));
    let (Some(_), Some(tag_value)) = (tag_subject, tag_value) else {
        return None;
    };
    if !is_bare_parameter_tag_reference(tp, tag_subject, parameter_symbol) {
        return None;
    }
    let tag = tag_value.as_string_literal().text().to_string();
    let tags = literal_tagged_union_tags(tp, parameter_name)?;
    if tags.contains(&tag) { Some(tag) } else { None }
}
