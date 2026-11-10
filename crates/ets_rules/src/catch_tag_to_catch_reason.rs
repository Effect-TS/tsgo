use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile, Symbol};
use ets_shim::checker::{LiteralValue, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{PipingFlowTransformation, ResultDispatch, TypeParser};
use rustc_hash::FxHashSet;

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::BRANCHING_ON_0_REASON_TAG_INSIDE_EFFECT_1_HAND_ROLLS_REASON_DISPATCH_USE_EFFECT_CATCH_REASON_OR_EFFECT_CATCH_REASONS_WHICH_RE_FAIL_UNMATCHED_REASONS_AUTOMATICALLY_EFFECT_CATCH_TAG_TO_CATCH_REASON;

// go: internal/rules/catch_tag_to_catch_reason.go:14 CatchTagToCatchReason c4ab7d33fe04fb61
pub static CATCH_TAG_TO_CATCH_REASON: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "catchTagToCatchReason",
    group: "style",
    description: "Suggests Effect.catchReason or Effect.catchReasons for handlers that re-fail unmatched reason._tag branches",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_catch_tag_to_catch_reason(ctx.type_parser, Some(source_file));
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

// go: internal/rules/catch_tag_to_catch_reason.go:33 CatchTagToCatchReasonBranch 8f788cc021ea7613
#[derive(Clone, Default)]
pub struct CatchTagToCatchReasonBranch {
    pub reason_tag: String,
    pub result: Option<P<Node>>,
    // UsesParameter reports whether Result reads the caught error parameter;
    // the parameter-use scan that validates the handler collects it.
    pub uses_parameter: bool,
}

// go: internal/rules/catch_tag_to_catch_reason.go:41 CatchTagToCatchReasonMatch 8cc25c606f1afca2
#[derive(Clone)]
pub struct CatchTagToCatchReasonMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub transformation: PipingFlowTransformation,
    pub callee: P<Node>,
    pub outer_tag: Option<P<Node>>,
    pub parameter_name: String,
    pub catch_method_name: &'static str,
    pub branches: Vec<CatchTagToCatchReasonBranch>,
    // Handler is the dispatching handler function.
    pub handler: P<Node>,
    // ParameterSymbol is the symbol of the handler's error parameter.
    pub parameter_symbol: P<Symbol>,
}

// go: internal/rules/catch_tag_to_catch_reason.go:56 catchTagToCatchReasonHandler 4bd89eb2aa856d04
#[derive(Clone)]
pub(crate) struct CatchTagToCatchReasonHandler {
    node: P<Node>,
    parameter_name: String,
    parameter_symbol: P<Symbol>,
    branches: Vec<CatchTagToCatchReasonBranch>,
}

// go: internal/rules/catch_tag_to_catch_reason.go:65 AnalyzeCatchTagToCatchReason 7572b0837add8d47
// AnalyzeCatchTagToCatchReason finds canonical reason-tag dispatch inside exact
// Effect.catchTag and Effect.catchTags transformations.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`), never nil.
pub fn analyze_catch_tag_to_catch_reason(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<CatchTagToCatchReasonMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        for (i, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(i).clone();
            let callee = transformation.callee();
            if tp.is_node_reference_to_effect_module_api(callee, "catchTag") {
                if let Some(m) = analyze_catch_tag_transformation(tp, sf, &transformation) {
                    matches.push(m);
                }
            } else if tp.is_node_reference_to_effect_module_api(callee, "catchTags")
                && let Some(m) = analyze_catch_tags_transformation(tp, sf, &transformation)
            {
                matches.push(m);
            }
        }
    }

    matches
}

// go: internal/rules/catch_tag_to_catch_reason.go:95 analyzeCatchTagTransformation e88dbd76e82ba0c2
pub(crate) fn analyze_catch_tag_transformation(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    transformation: &PipingFlowTransformation,
) -> Option<CatchTagToCatchReasonMatch> {
    if transformation.args().len() != 2 {
        return None;
    }

    let outer_tag = unwrap_transparent_expression(Some(*transformation.args().at(0)))?;
    if !ast::is_string_literal(outer_tag) {
        return None;
    }

    let handler = analyze_catch_tag_to_catch_reason_handler(tp, *transformation.args().at(1))?;

    Some(CatchTagToCatchReasonMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, transformation.callee()),
        transformation: transformation.clone(),
        callee: transformation.callee(),
        outer_tag: Some(outer_tag),
        parameter_name: handler.parameter_name,
        catch_method_name: "catchTag",
        branches: handler.branches,
        handler: handler.node,
        parameter_symbol: handler.parameter_symbol,
    })
}

// go: internal/rules/catch_tag_to_catch_reason.go:129 analyzeCatchTagsTransformation f082f7ae4a43ef47
pub(crate) fn analyze_catch_tags_transformation(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    transformation: &PipingFlowTransformation,
) -> Option<CatchTagToCatchReasonMatch> {
    if transformation.args().len() != 1 {
        return None;
    }

    let cases_node = unwrap_transparent_expression(Some(*transformation.args().at(0)))?;
    if cases_node.kind() != Kind::ObjectLiteralExpression {
        return None;
    }
    let cases = cases_node.as_object_literal_expression();

    let mut candidate: Option<CatchTagToCatchReasonHandler> = None;
    for &property_node in cases.properties().nodes() {
        if property_node.kind() != Kind::PropertyAssignment {
            continue;
        }
        let property = property_node.as_property_assignment();
        let initializer = property.initializer();
        if catch_tags_property_name(Some(property.name())).is_none() {
            continue;
        }

        let Some(handler) = analyze_catch_tag_to_catch_reason_handler(tp, initializer) else {
            continue;
        };
        candidate = Some(handler);
        break;
    }
    let candidate = candidate?;

    Some(CatchTagToCatchReasonMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, transformation.callee()),
        transformation: transformation.clone(),
        callee: transformation.callee(),
        outer_tag: None,
        parameter_name: candidate.parameter_name,
        catch_method_name: "catchTags",
        branches: candidate.branches,
        handler: candidate.node,
        parameter_symbol: candidate.parameter_symbol,
    })
}

// go: internal/rules/catch_tag_to_catch_reason.go:185 analyzeCatchTagToCatchReasonHandler 82b61415516d41f9
pub(crate) fn analyze_catch_tag_to_catch_reason_handler(
    tp: &mut TypeParser<'_>,
    handler_node: P<Node>,
) -> Option<CatchTagToCatchReasonHandler> {
    let returning = tp.parse_returning_dispatch_exported(handler_node)?;
    if returning.params().len() != 1 {
        return None;
    }
    let handler_node = returning.node();
    let body = returning.body();
    let parameter = *returning.params().at(0);
    let parameter_name = parameter.name()?;
    if parameter_name.kind() != Kind::Identifier {
        return None;
    }
    let dispatch: &ResultDispatch = returning.dispatch();
    if dispatch.branches().len() == 0 {
        return None;
    }
    let fallback = dispatch.fallback()?;
    let parameter_symbol = tp.get_symbol_at_location(parameter_name)?;
    let reason_tags = catch_reason_literal_tags(tp, parameter_name)?;

    let mut dispatch_refs: FxHashSet<P<Node>> = FxHashSet::default();
    let tag_subject = ResultDispatch::common_tag_subject(Some(dispatch), tp);
    if tag_subject.is_none()
        || !is_result_dispatch_tag_reference(tp, tag_subject, Some(parameter_symbol))
    {
        return None;
    }

    let mut branches =
        vec![CatchTagToCatchReasonBranch::default(); dispatch.branches().len() as usize];
    let mut seen_tags: FxHashSet<String> = FxHashSet::default();
    for (index, branch) in dispatch.branches().all() {
        let tag = result_dispatch_tag_value(branch.condition());
        let tagged = tag.is_some();
        let tag = tag.unwrap_or_default();
        let reference = catch_reason_tag_reference(
            tp,
            branch.condition().tag_subject(),
            Some(parameter_symbol),
        );
        let valid_reason_tag = reason_tags.contains(&tag);
        let duplicate_tag = seen_tags.contains(&tag);
        let root = reference?;
        if !tagged
            || !valid_reason_tag
            || duplicate_tag
            || !is_effect_expression(tp, Some(branch.result()))
        {
            return None;
        }
        dispatch_refs.insert(root);
        branches[index as usize] = CatchTagToCatchReasonBranch {
            reason_tag: tag.clone(),
            result: Some(branch.result()),
            ..Default::default()
        };
        seen_tags.insert(tag);
    }

    let fallback_param = catch_tag_re_fail_parameter(tp, Some(fallback), Some(parameter_symbol))?;

    let branch_parameter_uses = validate_catch_tag_parameter_uses(
        tp,
        body,
        parameter_symbol,
        &branches,
        Some(fallback_param),
        &dispatch_refs,
    )?;
    for (i, branch) in branches.iter_mut().enumerate() {
        branch.uses_parameter = branch_parameter_uses[i];
    }

    Some(CatchTagToCatchReasonHandler {
        node: handler_node,
        parameter_name: parameter_name.as_identifier().text().to_string(),
        parameter_symbol,
        branches,
    })
}

// go: internal/rules/catch_tag_to_catch_reason.go:251 catchReasonLiteralTags 90b8883133940cee
pub(crate) fn catch_reason_literal_tags(
    tp: &mut TypeParser<'_>,
    parameter_name: P<Node>,
) -> Option<FxHashSet<String>> {
    let parameter_type = tp.get_type_at_location(parameter_name)?;
    let mut reason_type =
        tp.checker().get_type_of_property_of_type_exported(parameter_type, "reason");
    if reason_type.is_none() {
        reason_type = tp.get_type_of_property_by_name(parameter_type, "reason");
    }
    let reason_type = reason_type?;
    if !reason_type.flags().intersects(TypeFlags::Union) {
        return None;
    }

    let mut tags = FxHashSet::default();
    for member in tp.unroll_union_members(reason_type).unwrap_or_default() {
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

// go: internal/rules/catch_tag_to_catch_reason.go:288 catchReasonTagReference 41f7c220b804fe84
pub(crate) fn catch_reason_tag_reference(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
    parameter_symbol: Option<P<Symbol>>,
) -> Option<P<Node>> {
    let node = unwrap_transparent_expression(node)?;
    if node.kind() != Kind::PropertyAccessExpression {
        return None;
    }
    let reason_access = node.as_property_access_expression();
    if reason_access.name().text() != "reason" {
        return None;
    }
    let root = unwrap_transparent_expression(Some(reason_access.expression()))?;
    if root.kind() != Kind::Identifier
        || !same_catch_reason_symbol(tp, Some(root), parameter_symbol)
    {
        return None;
    }
    Some(root)
}

// go: internal/rules/catch_tag_to_catch_reason.go:304 catchTagReFailParameter d4bec453f20f8e56
pub(crate) fn catch_tag_re_fail_parameter(
    tp: &mut TypeParser<'_>,
    expression: Option<P<Node>>,
    parameter_symbol: Option<P<Symbol>>,
) -> Option<P<Node>> {
    let expression = unwrap_transparent_expression(expression)?;
    if expression.kind() == Kind::CallExpression {
        let call = expression.as_call_expression();
        let arguments = call.arguments().nodes();
        if arguments.len() == 1
            && tp.is_node_reference_to_effect_module_api(call.expression(), "fail")
        {
            let argument = unwrap_transparent_expression(Some(arguments[0]));
            if let Some(argument) = argument
                && argument.kind() == Kind::Identifier
                && same_catch_reason_symbol(tp, Some(argument), parameter_symbol)
            {
                return Some(argument);
            }
        }
    }
    None
}

// go: internal/rules/catch_tag_to_catch_reason.go:322 validateCatchTagParameterUses 84d1c31cf7f6c690
//
// Rust: Go's `(uses, valid)` result is `Some(uses)` when valid.
pub(crate) fn validate_catch_tag_parameter_uses(
    tp: &mut TypeParser<'_>,
    body: P<Node>,
    parameter_symbol: P<Symbol>,
    branches: &[CatchTagToCatchReasonBranch],
    fallback_param: Option<P<Node>>,
    dispatch_refs: &FxHashSet<P<Node>>,
) -> Option<Vec<bool>> {
    let mut allowed: FxHashSet<P<Node>> = FxHashSet::default();
    for &node in dispatch_refs {
        allowed.insert(node);
    }
    if let Some(fallback_param) = fallback_param {
        allowed.insert(fallback_param);
    }

    let mut branch_uses = vec![false; branches.len()];
    let mut valid = true;

    // Rust: Go's recursive `walkBody` closure is a nested function.
    fn walk_body(
        tp: &mut TypeParser<'_>,
        node: P<Node>,
        parameter_symbol: P<Symbol>,
        branches: &[CatchTagToCatchReasonBranch],
        allowed: &FxHashSet<P<Node>>,
        branch_uses: &mut [bool],
        valid: &mut bool,
    ) -> bool {
        if !*valid {
            return true;
        }
        if node.kind() == Kind::Identifier
            && same_catch_reason_symbol(tp, Some(node), Some(parameter_symbol))
        {
            if allowed.contains(&node) {
                return false;
            }
            let branch_index = catch_reason_recovery_branch_index(node, branches);
            if branch_index >= 0 {
                branch_uses[branch_index as usize] = true;
                return false;
            }
            *valid = false;
            return true;
        }
        node.for_each_child(&mut |child| {
            walk_body(tp, child, parameter_symbol, branches, allowed, branch_uses, valid)
        });
        false
    }
    walk_body(tp, body, parameter_symbol, branches, &allowed, &mut branch_uses, &mut valid);
    if valid { Some(branch_uses) } else { None }
}

// go: internal/rules/catch_tag_to_catch_reason.go:365 catchReasonRecoveryBranchIndex f8e06d3875c7560a
pub(crate) fn catch_reason_recovery_branch_index(
    node: P<Node>,
    branches: &[CatchTagToCatchReasonBranch],
) -> i64 {
    for (i, branch) in branches.iter().enumerate() {
        if let Some(expression) = branch.result
            && node.pos() >= expression.pos()
            && node.end() <= expression.end()
        {
            return i as i64;
        }
    }
    -1
}

// go: internal/rules/catch_tag_to_catch_reason.go:375 isEffectExpression b7a91be637846585
pub(crate) fn is_effect_expression(tp: &mut TypeParser<'_>, expression: Option<P<Node>>) -> bool {
    tp.effect_type_at_location(expression).is_some()
}

// go: internal/rules/catch_tag_to_catch_reason.go:379 catchTagsPropertyName d37709e4bd52b4db
pub(crate) fn catch_tags_property_name(name: Option<P<Node>>) -> Option<String> {
    let name = name?;
    match name.kind() {
        Kind::Identifier => Some(name.as_identifier().text().to_string()),
        Kind::StringLiteral => Some(name.as_string_literal().text().to_string()),
        _ => None,
    }
}

// go: internal/rules/catch_tag_to_catch_reason.go:393 sameCatchReasonSymbol 8011d80abd7bf5f0
pub(crate) fn same_catch_reason_symbol(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
    expected: Option<P<Symbol>>,
) -> bool {
    let actual = tp.get_symbol_at_location(node);
    let (Some(actual), Some(expected)) = (actual, expected) else {
        return false;
    };
    tp.checker().get_symbol_if_same_reference(actual, expected).is_some()
}
