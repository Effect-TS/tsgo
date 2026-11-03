use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::PipingFlowTransformationReplacement;
use ets_rules::IsTaggedOnTaggedUnionMatch;
use ets_shim::core::alloc_str;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::PipingFlowTransformation;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::PREDICATE_IS_TAGGED_ACCEPTS_ANY_VALUE_SO_ON_THIS_TAGGED_UNION_IT_DOES_NOT_CHECK_THE_TAG_AGAINST_THE_MEMBERS_AND_NARROWS_LESS_THAN_PREDICATE_IS_TAGGED_STRICT_EFFECT_IS_TAGGED_ON_TAGGED_UNION;

// go: internal/fixables/is_tagged_on_tagged_union.go:13 IsTaggedOnTaggedUnionFix db59ed7db8aa0d46
pub static IS_TAGGED_ON_TAGGED_UNION_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "isTaggedOnTaggedUnion",
    description: "Replace Predicate.isTagged with Predicate.isTaggedStrict or the data type's guard",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["isTaggedOnTaggedUnion_fix"],
    run: run_is_tagged_on_tagged_union_fix,
});

// go: internal/fixables/is_tagged_on_tagged_union.go:23 runIsTaggedOnTaggedUnionFix 05c568c55417962d
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
        let transformation = m.flow.transformations().at(m.index).clone();
        let mut fix = is_tagged_guard_fix(ctx, &m, &transformation);
        if fix.is_none() {
            fix = is_tagged_strict_fix(ctx, &m, &transformation);
        }
        let Some(fix) = fix else {
            continue;
        };
        if let Some(action) = ctx.new_fix_action(fix) {
            return vec![action];
        }
    }
    Vec::new()
}

// go: internal/fixables/is_tagged_on_tagged_union.go:50 isTaggedGuardFix 326fd0067063868e
// isTaggedGuardFix replaces isTagged with the is<Tag> guard of Effect's
// Option, Exit or Result, when the union is that data type and its module is
// reachable.
fn is_tagged_guard_fix(
    ctx: &mut Context<'_, '_>,
    m: &IsTaggedOnTaggedUnionMatch,
    transformation: &PipingFlowTransformation,
) -> Option<FixAction<'static>> {
    if m.module.is_empty() {
        return None;
    }
    let member = tagged_data_type_guard(m.module, alloc_str(&format!("is{}", m.tag)));
    let preferred = ets_typeparser::find_module_reference(Some(m.source_file), m.module);
    let guard = resolve_module_member(ctx, transformation.callee(), &preferred, &member)?;
    let description = format!("Replace with {}.{}", m.module, member.name);
    let m = m.clone();
    let transformation = transformation.clone();
    Some(FixAction {
        description,
        run: Box::new(move |tracker| {
            if m.applied {
                let callee = guard.expression(tracker, &member);
                tracker.replace_piping_flow_transformation(
                    Some(m.source_file),
                    Some(&transformation),
                    PipingFlowTransformationReplacement {
                        callee: Some(callee),
                        type_arguments: None,
                        arguments: None,
                    },
                );
                return;
            }
            // An unapplied isTagged(tag) takes the tag as its input, so the guard
            // replaces the call and its input.
            let replacement = guard.expression(tracker, &member);
            tracker.replace_piping_flow_prefix(
                Some(m.source_file),
                Some(&m.flow),
                m.index + 1,
                Some(replacement),
            );
        }),
    })
}

// go: internal/fixables/is_tagged_on_tagged_union.go:74 isTaggedStrictFix b319403bb2cd3f79
// isTaggedStrictFix replaces isTagged with Predicate.isTaggedStrict, written
// with the qualifier the callee has, keeping the arguments. It is offered only
// when the installed Effect exports isTaggedStrict.
#[expect(
    clippy::redundant_closure_for_method_calls,
    reason = "the method path is not general over the parser's lifetime"
)]
fn is_tagged_strict_fix(
    ctx: &mut Context<'_, '_>,
    m: &IsTaggedOnTaggedUnionMatch,
    transformation: &PipingFlowTransformation,
) -> Option<FixAction<'static>> {
    let member = ModuleMember {
        name: "isTaggedStrict",
        is_module: Box::new(|tp, node| tp.is_expression_effect_predicate_module(node)),
        is_member: Box::new(|tp, node| {
            tp.is_node_reference_to_effect_predicate_module_api(node, "isTaggedStrict")
        }),
    };
    let module = ets_typeparser::find_module_reference(Some(m.source_file), "Predicate");
    let (strict, named_imports) =
        callee_module_member_reference(ctx, transformation.callee(), &module, &member);
    let strict = strict?;
    let m = m.clone();
    let transformation = transformation.clone();
    Some(FixAction {
        description: "Replace with Predicate.isTaggedStrict".to_string(),
        run: Box::new(move |tracker| {
            let mut arguments = None;
            if transformation.args().len() > 0 {
                let nodes = transformation
                    .args()
                    .all()
                    .map(|(_, &argument)| tracker.deep_clone_node(Some(argument)).unwrap())
                    .collect();
                arguments = Some(tracker.new_node_list(nodes));
            }
            let callee = strict.expression(tracker, &member);
            let replaced = tracker.replace_piping_flow_transformation(
                Some(m.source_file),
                Some(&transformation),
                PipingFlowTransformationReplacement {
                    callee: Some(callee),
                    type_arguments: None,
                    arguments,
                },
            );
            if replaced && let Some(named_imports) = named_imports {
                let elements = named_imports.as_named_imports().elements();
                let specifier =
                    tracker.new_import_specifier(false, None, tracker.new_identifier(member.name));
                tracker.insert_import_specifier_at_index(
                    m.source_file,
                    specifier,
                    named_imports,
                    elements.nodes().len(),
                );
            }
        }),
    })
}

// go: internal/fixables/is_tagged_on_tagged_union.go:107 taggedDataTypeGuard 48049dec0dc624e6
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
        _ => ModuleMember {
            name,
            is_module: Box::new(|tp, node| tp.is_expression_effect_result_module(node)),
            is_member: Box::new(move |tp, node| {
                tp.is_node_reference_to_effect_result_module_api(node, name)
            }),
        },
    }
}
