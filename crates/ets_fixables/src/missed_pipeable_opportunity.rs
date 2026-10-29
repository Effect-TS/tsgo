use std::sync::LazyLock;

use ets_etscore::ResolvedEffectPluginOptions;
use ets_fixable::{Context, FixAction, Fixable};
use ets_readonly::Slice;
use ets_rewriter::Tracker;
use ets_rules::MissedPipeableOpportunityMatch;
use ets_shim::P;
use ets_shim::ast::{self, Node, NodeFlags, SourceFile};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::{PipingFlow, PipingFlowTransformation, TransformationKind};

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_NESTED_CALL_STRUCTURE_HAS_A_PIPEABLE_FORM_0_PIPE_REPRESENTS_THE_SAME_CALL_SEQUENCE_IN_PIPE_STYLE_AND_MAY_BE_EASIER_TO_READ_EFFECT_MISSED_PIPEABLE_OPPORTUNITY;

// go: internal/fixables/missed_pipeable_opportunity.go:14 MissedPipeableOpportunityFix 35af22400f2b40b0
pub static MISSED_PIPEABLE_OPPORTUNITY_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "missedPipeableOpportunity",
    description: "Convert to pipe style",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["missedPipeableOpportunity_fix"],
    run: run_missed_pipeable_opportunity_fix,
});

// go: internal/fixables/missed_pipeable_opportunity.go:22 runMissedPipeableOpportunityFix 0c32f2acead3b839
fn run_missed_pipeable_opportunity_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let mut min_arg_count = 2;
    let effect_config = ctx.options;
    if effect_config.is_some() {
        min_arg_count = ResolvedEffectPluginOptions::get_pipeable_min_arg_count(effect_config);
    }

    let matches =
        ets_rules::analyze_missed_pipeable_opportunity(&mut ctx.type_parser, sf, min_arg_count);

    let mut matched: Option<MissedPipeableOpportunityMatch> = None;
    for m in matches {
        let diag_range = m.location;
        if diag_range.intersects(ctx.span) || ctx.span.contained_by(diag_range) {
            matched = Some(m);
            break;
        }
    }
    let Some(matched) = matched else {
        return Vec::new();
    };

    if let Some(action) = ctx.new_fix_action(FixAction {
        description: "Convert to pipe style".to_string(),
        run: Box::new(move |tracker| {
            build_missed_pipeable_replacement(tracker, sf, &matched);
        }),
    }) {
        return vec![action];
    }
    Vec::new()
}

// go: internal/fixables/missed_pipeable_opportunity.go:59 buildMissedPipeableReplacement a5c287105552e96c
// buildMissedPipeableReplacement builds the .pipe() replacement and applies it via tracker.ReplaceNode.
fn build_missed_pipeable_replacement(
    tracker: &mut Tracker,
    sf: P<SourceFile>,
    m: &MissedPipeableOpportunityMatch,
) {
    let flow = &m.flow;

    let Some(subject_input) =
        PipingFlow::transformation_input_node(Some(flow), m.pipeable_start_index)
    else {
        return;
    };
    let subject_node = tracker.deep_clone_node(Some(subject_input)).unwrap();

    // Build pipe arguments from transformations
    let mut pipe_args = Vec::with_capacity(m.pipeable_transformations.len());
    for t in &m.pipeable_transformations {
        let arg = if t.args().len() > 0 {
            // It's a function call like Effect.map((x) => x + 1) -> create callee(args...)
            let cloned_callee = tracker.deep_clone_node(Some(t.callee())).unwrap();
            let mut cloned_args = Vec::with_capacity(t.args().len() as usize);
            for (_, &a) in t.args().all() {
                cloned_args.push(tracker.deep_clone_node(Some(a)).unwrap());
            }
            tracker.new_call_expression(
                cloned_callee,
                None,
                None,
                tracker.new_node_list(cloned_args),
                NodeFlags::None,
            )
        } else {
            // It's a constant like Effect.asVoid
            tracker.deep_clone_node(Some(t.callee())).unwrap()
        };
        pipe_args.push(arg);
    }

    // Create subject.pipe(arg1, arg2, ...)
    let pipe_access = tracker.new_property_access_expression(
        subject_node,
        None,
        tracker.new_identifier("pipe"),
        NodeFlags::None,
    );
    let pipe_call = tracker.new_call_expression(
        pipe_access,
        None,
        None,
        tracker.new_node_list(pipe_args),
        NodeFlags::None,
    );

    // Wrap with "after" transformations if any
    let after_transformations = ets_readonly::new(m.after_transformations.as_deref());
    let replacement_node = if after_transformations.len() > 0 {
        wrap_with_after_transformations(tracker, pipe_call, after_transformations)
    } else {
        pipe_call
    };

    ast::set_parent_in_children(replacement_node);
    tracker.replace_node(sf, Some(m.node), Some(replacement_node), None);
}

// go: internal/fixables/missed_pipeable_opportunity.go:108 wrapWithAfterTransformations 8209e8af6acc01ea
// wrapWithAfterTransformations wraps an inner node with the "after" transformations.
// For "call" kind: callee(innerNode)
// For "callWithOptionals" kind: callee(innerNode, args...)
// For "pipe"/"pipeable" kind with args: callee(args...)(innerNode) (curried)
// For "pipe"/"pipeable" kind without args: callee(innerNode)
fn wrap_with_after_transformations(
    tracker: &Tracker,
    inner: P<Node>,
    transformations: Slice<'_, PipingFlowTransformation>,
) -> P<Node> {
    let mut result = inner;
    for (_, t) in transformations.all() {
        if t.kind() == TransformationKind::EffectFn
            || t.kind() == TransformationKind::EffectFnUntraced
        {
            continue;
        }

        let cloned_callee = tracker.deep_clone_node(Some(t.callee())).unwrap();

        match t.kind() {
            TransformationKind::Call => {
                // Single-arg call: callee(result)
                result = tracker.new_call_expression(
                    cloned_callee,
                    None,
                    None,
                    tracker.new_node_list(vec![result]),
                    NodeFlags::None,
                );
            }
            TransformationKind::CallWithOptionals => {
                let mut call_args = vec![result];
                for (_, &a) in t.args().all() {
                    call_args.push(tracker.deep_clone_node(Some(a)).unwrap());
                }
                result = tracker.new_call_expression(
                    cloned_callee,
                    None,
                    None,
                    tracker.new_node_list(call_args),
                    NodeFlags::None,
                );
            }
            _ => {
                // Pipe or pipeable transformation
                if t.args().len() > 0 {
                    // Curried form: callee(args...)(result)
                    let mut cloned_args = Vec::with_capacity(t.args().len() as usize);
                    for (_, &a) in t.args().all() {
                        cloned_args.push(tracker.deep_clone_node(Some(a)).unwrap());
                    }
                    let curried_call = tracker.new_call_expression(
                        cloned_callee,
                        None,
                        None,
                        tracker.new_node_list(cloned_args),
                        NodeFlags::None,
                    );
                    result = tracker.new_call_expression(
                        curried_call,
                        None,
                        None,
                        tracker.new_node_list(vec![result]),
                        NodeFlags::None,
                    );
                } else {
                    // Constant: callee(result)
                    result = tracker.new_call_expression(
                        cloned_callee,
                        None,
                        None,
                        tracker.new_node_list(vec![result]),
                        NodeFlags::None,
                    );
                }
            }
        }
    }
    result
}
