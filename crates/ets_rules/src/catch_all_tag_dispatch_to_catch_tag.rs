use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile, Symbol};
use ets_shim::checker::{LiteralValue, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{PipingFlowTransformation, ResultDispatch, TypeParser};
use rustc_hash::FxHashSet;

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::BRANCHING_ON_0_TAG_INSIDE_EFFECT_1_HAND_ROLLS_TAGGED_ERROR_DISPATCH_USE_EFFECT_CATCH_TAG_OR_EFFECT_CATCH_TAGS_WHICH_RE_FAIL_UNMATCHED_ERRORS_AUTOMATICALLY_EFFECT_CATCH_ALL_TAG_DISPATCH_TO_CATCH_TAG;

// go: internal/rules/catch_all_tag_dispatch_to_catch_tag.go:14 CatchAllTagDispatchToCatchTag 4159688f4fa4d3fd
pub static CATCH_ALL_TAG_DISPATCH_TO_CATCH_TAG: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "catchAllTagDispatchToCatchTag",
    group: "style",
    description: "Suggests Effect.catchTag or Effect.catchTags for catch-all handlers that re-fail unmatched tagged errors",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches =
            analyze_catch_all_tag_dispatch_to_catch_tag(ctx.type_parser, Some(source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[&m.parameter_name, m.catch_method_name],
            ));
        }
        diagnostics
    },
});

// go: internal/rules/catch_all_tag_dispatch_to_catch_tag.go:33 CatchAllTagDispatchBranch 6fe15dd4096f1fbe
#[derive(Clone)]
pub struct CatchAllTagDispatchBranch {
    pub tag: String,
    pub result: P<Node>,
    pub uses_parameter: bool,
}

// go: internal/rules/catch_all_tag_dispatch_to_catch_tag.go:39 CatchAllTagDispatchMatch 0e5c00caf0b28b4c
#[derive(Clone)]
pub struct CatchAllTagDispatchMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub transformation: PipingFlowTransformation,
    pub callee: P<Node>,
    pub parameter_name: String,
    pub catch_method_name: &'static str,
    pub branches: Vec<CatchAllTagDispatchBranch>,
    pub can_fix: bool,
}

// go: internal/rules/catch_all_tag_dispatch_to_catch_tag.go:50 AnalyzeCatchAllTagDispatchToCatchTag ce281d04e7a98615
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`), never nil.
pub fn analyze_catch_all_tag_dispatch_to_catch_tag(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<CatchAllTagDispatchMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        for (index, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(index).clone();
            if transformation.args().len() != 1 {
                continue;
            }
            let Some(catch_method) = catch_all_tag_dispatch_method(tp, transformation.callee())
            else {
                continue;
            };
            let Some((parameter_name, branches, can_fix)) =
                analyze_catch_all_tag_dispatch_handler(tp, *transformation.args().at(0))
            else {
                continue;
            };
            matches.push(CatchAllTagDispatchMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, transformation.callee()),
                callee: transformation.callee(),
                transformation,
                parameter_name,
                catch_method_name: catch_method,
                branches,
                can_fix,
            });
        }
    }
    matches
}

// go: internal/rules/catch_all_tag_dispatch_to_catch_tag.go:81 catchAllTagDispatchMethod 5ddf7f07ba2bfb30
pub(crate) fn catch_all_tag_dispatch_method(
    tp: &mut TypeParser<'_>,
    callee: P<Node>,
) -> Option<&'static str> {
    if tp.is_node_reference_to_effect_module_api(callee, "catch") {
        return Some("catch");
    }
    if tp.is_node_reference_to_effect_module_api(callee, "catchAll") {
        return Some("catchAll");
    }
    None
}

// go: internal/rules/catch_all_tag_dispatch_to_catch_tag.go:91 analyzeCatchAllTagDispatchHandler b747896c4d1082e0
//
// Rust: Go's `(name, branches, canFix, ok)` result is `Some((name, branches, canFix))` when ok.
pub(crate) fn analyze_catch_all_tag_dispatch_handler(
    tp: &mut TypeParser<'_>,
    handler_node: P<Node>,
) -> Option<(String, Vec<CatchAllTagDispatchBranch>, bool)> {
    let returning = tp.parse_returning_dispatch(handler_node)?;
    let dispatch: &ResultDispatch = returning.dispatch();
    if returning.params().len() != 1 || dispatch.branches().len() == 0 {
        return None;
    }
    let fallback = dispatch.fallback()?;
    let parameter = *returning.params().at(0);
    let parameter_name = parameter.name()?;
    if parameter_name.kind() != Kind::Identifier {
        return None;
    }
    let parameter_symbol = tp.get_symbol_at_location(parameter_name)?;
    let tags = literal_tagged_union_tags(tp, parameter_name)?;
    let tag_subject = ResultDispatch::common_tag_subject(Some(dispatch), tp);
    if tag_subject.is_none() || !is_bare_parameter_tag_reference(tp, tag_subject, parameter_symbol)
    {
        return None;
    }

    let mut branches = Vec::with_capacity(dispatch.branches().len() as usize);
    let mut seen: FxHashSet<String> = FxHashSet::default();
    for (_, branch) in dispatch.branches().all() {
        let tag = result_dispatch_tag_value(branch.condition());
        let tagged = tag.is_some();
        let tag = tag.unwrap_or_default();
        let valid_tag = tags.contains(&tag);
        let duplicate = seen.contains(&tag);
        if !tagged || !valid_tag || duplicate || !is_effect_expression(tp, Some(branch.result())) {
            return None;
        }
        seen.insert(tag.clone());
        branches.push(CatchAllTagDispatchBranch {
            tag,
            result: branch.result(),
            uses_parameter: conditional_refail_node_contains_parameter(
                tp,
                Some(branch.result()),
                Some(parameter_symbol),
            ),
        });
    }
    catch_tag_re_fail_parameter(tp, Some(fallback), Some(parameter_symbol))?;
    let can_fix = returning.node().kind() == Kind::ArrowFunction
        && !tp.checker().is_symbol_assigned(parameter_symbol);
    Some((parameter_name.as_identifier().text().to_string(), branches, can_fix))
}

// go: internal/rules/catch_all_tag_dispatch_to_catch_tag.go:131 literalTaggedUnionTags 931ed3a1363bb4f2
pub(crate) fn literal_tagged_union_tags(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
) -> Option<FxHashSet<String>> {
    let type_ = tp.get_type_at_location(node)?;
    if !type_.flags().intersects(TypeFlags::Union) {
        return None;
    }
    let mut tags = FxHashSet::default();
    for member in tp.unroll_union_members(type_).unwrap_or_default() {
        let mut tag_type = tp.checker().get_type_of_property_of_type_exported(member, "_tag");
        if tag_type.is_none() {
            tag_type = tp.get_type_of_property_by_name(member, "_tag");
        }
        let tag_type = tag_type?;
        if !tag_type.flags().intersects(TypeFlags::StringLiteral) {
            return None;
        }
        let Some(LiteralValue::String(tag)) = tag_type.as_literal_type().value() else {
            return None;
        };
        tags.insert(tag.to_string());
    }
    if tags.is_empty() { None } else { Some(tags) }
}

// go: internal/rules/catch_all_tag_dispatch_to_catch_tag.go:157 isBareParameterTagReference 499ac373ad000a8a
pub(crate) fn is_bare_parameter_tag_reference(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
    parameter_symbol: P<Symbol>,
) -> bool {
    let Some(node) = unwrap_transparent_expression(node) else {
        return false;
    };
    if node.kind() != Kind::Identifier {
        return false;
    }
    same_catch_reason_symbol(tp, Some(node), Some(parameter_symbol))
}
