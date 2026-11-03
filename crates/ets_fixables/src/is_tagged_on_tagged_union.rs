use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::{PipingFlowTransformationReplacement, Tracker};
use ets_rules::IsTaggedOnTaggedUnionMatch;
use ets_shim::P;
use ets_shim::ast::{self, Kind, Node, NodeFlags, OperatorPrecedence, TokenFlags};
use ets_shim::core::alloc_str;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::PipingFlow;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::PREDICATE_IS_TAGGED_ACCEPTS_ANY_VALUE_SO_ON_THIS_TAGGED_UNION_IT_NEITHER_CHECKS_THE_TAG_AGAINST_THE_MEMBERS_NOR_NARROWS_AS_A_TAG_COMPARISON_DOES_EFFECT_IS_TAGGED_ON_TAGGED_UNION;

// go: internal/fixables/is_tagged_on_tagged_union.go:13 IsTaggedOnTaggedUnionFix 0704f78670fecfdd
pub static IS_TAGGED_ON_TAGGED_UNION_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "isTaggedOnTaggedUnion",
    description: "Replace Predicate.isTagged with a _tag comparison or the data type's guard",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["isTaggedOnTaggedUnion_fix"],
    run: run_is_tagged_on_tagged_union_fix,
});

// go: internal/fixables/is_tagged_on_tagged_union.go:23 runIsTaggedOnTaggedUnionFix 4a99543bfcbc67bd
fn run_is_tagged_on_tagged_union_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_is_tagged_on_tagged_union(&mut ctx.type_parser, Some(source_file)) {
        // A tag outside the union has no replacement that keeps the meaning.
        if !m.known_tag {
            continue;
        }
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        let input = PipingFlow::transformation_input_node(Some(&m.flow), m.index);

        let mut guard = None;
        let mut member = None;
        if !m.module.is_empty() {
            let guard_member = tagged_data_type_guard(m.module, alloc_str(&format!("is{}", m.tag)));
            let callee = m.flow.transformations().at(m.index).callee();
            let preferred = ets_typeparser::find_module_reference(Some(m.source_file), m.module);
            guard = resolve_module_member(ctx, callee, &preferred, &guard_member);
            member = Some(guard_member);
        }
        let mut description = "Replace with a _tag comparison".to_string();
        if guard.is_some()
            && let Some(member) = &member
        {
            description = format!("Replace with {}.{}", m.module, member.name);
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description,
            run: Box::new(move |tracker| match (&guard, &member) {
                (Some(guard), Some(member)) if m.applied && input.is_some() => {
                    let call = tracker.new_call_expression(
                        guard.expression(tracker, member),
                        None,
                        None,
                        tracker.new_node_list(vec![tracker.deep_clone_node(input).unwrap()]),
                        NodeFlags::None,
                    );
                    tracker.replace_piping_flow_prefix(
                        Some(m.source_file),
                        Some(&m.flow),
                        m.index + 1,
                        Some(call),
                    );
                }
                _ if m.applied && input.is_some() => {
                    replace_is_tagged_with_tag_comparison(tracker, &m, input.unwrap());
                }
                _ if m.applied => {
                    // A pipe step after the first has no input node, so the predicate becomes the step.
                    let transformation = m.flow.transformations().at(m.index).clone();
                    let callee =
                        is_tagged_predicate(tracker, guard.as_ref(), member.as_ref(), &m.tag);
                    tracker.replace_piping_flow_transformation(
                        Some(m.source_file),
                        Some(&transformation),
                        PipingFlowTransformationReplacement {
                            callee: Some(callee),
                            type_arguments: None,
                            arguments: None,
                        },
                    );
                }
                _ => {
                    let mut predicate =
                        is_tagged_predicate(tracker, guard.as_ref(), member.as_ref(), &m.tag);
                    // A step followed by others becomes the argument of the next one.
                    if guard.is_none()
                        && m.index == m.flow.transformations().len() - 1
                        && !position_accepts_any_expression(m.flow.node())
                    {
                        predicate = tracker.new_parenthesized_expression(predicate);
                    }
                    tracker.replace_piping_flow_prefix(
                        Some(m.source_file),
                        Some(&m.flow),
                        m.index + 1,
                        Some(predicate),
                    );
                }
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}

// go: internal/fixables/is_tagged_on_tagged_union.go:79 isTaggedPredicate 4c5a827e4d34361c
// isTaggedPredicate is the refinement that stands for isTagged(tag): the
// data type's guard, or value => value._tag === tag.
//
// Rust: Go's guard and member are set together, so a guard comes with its member.
fn is_tagged_predicate(
    tracker: &Tracker,
    guard: Option<&ModuleMemberReference>,
    member: Option<&ModuleMember>,
    tag: &str,
) -> P<Node> {
    if let (Some(guard), Some(member)) = (guard, member) {
        return guard.expression(tracker, member);
    }
    let parameter = tracker.new_parameter_declaration(
        None,
        None,
        tracker.new_identifier("value"),
        None,
        None,
        None,
    );
    tracker.new_arrow_function(
        None,
        None,
        Some(tracker.new_node_list(vec![parameter])),
        None,
        None,
        Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
        Some(tag_comparison(
            tracker,
            tracker.new_identifier("value"),
            Kind::EqualsEqualsEqualsToken,
            tag,
        )),
    )
}

// go: internal/fixables/is_tagged_on_tagged_union.go:92 replaceIsTaggedWithTagComparison e37901e381b57467
// replaceIsTaggedWithTagComparison writes input._tag === tag in place of an
// applied isTagged, or input._tag !== tag in place of its negation. The `!`
// is not part of the flow, so a negated test is replaced as a node.
fn replace_is_tagged_with_tag_comparison(
    tracker: &mut Tracker,
    m: &IsTaggedOnTaggedUnionMatch,
    input: P<Node>,
) {
    let mut object = tracker.deep_clone_node(Some(input)).unwrap();
    if !ast::is_left_hand_side_expression(input) {
        object = tracker.new_parenthesized_expression(object);
    }
    let last = m.index == m.flow.transformations().len() - 1;
    let position = m.flow.node();
    if let Some(parent) = position.parent()
        && last
        && parent.kind() == Kind::PrefixUnaryExpression
        && parent.as_prefix_unary_expression().operator() == Kind::ExclamationToken
    {
        let mut comparison =
            tag_comparison(tracker, object, Kind::ExclamationEqualsEqualsToken, &m.tag);
        if tag_comparison_needs_parentheses(parent) {
            comparison = tracker.new_parenthesized_expression(comparison);
        }
        tracker.replace_node(m.source_file, Some(parent), Some(comparison), None);
        return;
    }
    let mut comparison = tag_comparison(tracker, object, Kind::EqualsEqualsEqualsToken, &m.tag);
    // A step followed by others becomes the argument of the next one.
    if last && tag_comparison_needs_parentheses(position) {
        comparison = tracker.new_parenthesized_expression(comparison);
    }
    tracker.replace_piping_flow_prefix(
        Some(m.source_file),
        Some(&m.flow),
        m.index + 1,
        Some(comparison),
    );
}

// go: internal/fixables/is_tagged_on_tagged_union.go:116 tagComparison 3116f271455ecae2
fn tag_comparison(tracker: &Tracker, object: P<Node>, operator: Kind, tag: &str) -> P<Node> {
    let access = tracker.new_property_access_expression(
        object,
        None,
        tracker.new_identifier("_tag"),
        NodeFlags::None,
    );
    tracker.new_binary_expression(
        None,
        access,
        None,
        tracker.new_token(operator),
        tracker.new_string_literal(alloc_str(tag), TokenFlags::None),
    )
}

// go: internal/fixables/is_tagged_on_tagged_union.go:123 tagComparisonNeedsParentheses a8b73b20f8019448
// tagComparisonNeedsParentheses reports whether an equality expression at
// position binds less tightly than the expression it is an operand of.
fn tag_comparison_needs_parentheses(position: P<Node>) -> bool {
    let Some(parent) = position.parent() else {
        return false;
    };
    if position_accepts_any_expression(position) {
        return false;
    }
    if parent.kind() == Kind::BinaryExpression {
        return ast::get_binary_operator_precedence(
            parent.as_binary_expression().operator_token().kind(),
        ) >= OperatorPrecedence::Equality;
    }
    if ast::is_expression(parent) {
        return ast::get_expression_precedence(parent) > OperatorPrecedence::Equality;
    }
    false
}

// go: internal/fixables/is_tagged_on_tagged_union.go:138 taggedDataTypeGuard 5bd36204861d943d
// taggedDataTypeGuard is the is<Tag> guard exported by one of the modules
// TypeParser.TaggedDataTypeModule returns.
//
// Rust: the predicates get the type parser as an argument, so Go's `ctx` parameter drops.
#[expect(
    clippy::redundant_closure_for_method_calls,
    reason = "the method path is not general over the parser's lifetime"
)]
fn tagged_data_type_guard(module: &str, name: &'static str) -> ModuleMember {
    match module {
        "Option" => ModuleMember {
            name,
            is_module: Box::new(|tp, node| tp.is_expression_effect_option_module(node)),
            is_member: Box::new(move |tp, node| {
                tp.is_node_reference_to_effect_option_module_api(node, name)
            }),
        },
        "Exit" => ModuleMember {
            name,
            is_module: Box::new(|tp, node| tp.is_expression_effect_exit_module(node)),
            is_member: Box::new(move |tp, node| {
                tp.is_node_reference_to_effect_exit_module_api(node, name)
            }),
        },
        "Result" => ModuleMember {
            name,
            is_module: Box::new(|tp, node| tp.is_expression_effect_result_module(node)),
            is_member: Box::new(move |tp, node| {
                tp.is_node_reference_to_effect_result_module_api(node, name)
            }),
        },
        _ => ModuleMember {
            name,
            is_module: Box::new(|tp, node| tp.is_expression_effect_either_module(node)),
            is_member: Box::new(move |tp, node| {
                tp.is_node_reference_to_effect_either_module_api(node, name)
            }),
        },
    }
}
