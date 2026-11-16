use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::{PipingFlowTransformationReplacement, Tracker};
use ets_rules::{CatchTagToCatchReasonBranch, CatchTagToCatchReasonMatch};
use ets_shim::P;
use ets_shim::ast::{Kind, Node, NodeList, SourceFile, SymbolFlags, TokenFlags};
use ets_shim::checker::Checker;
use ets_shim::core::alloc_str;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use rustc_hash::FxHashSet;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::BRANCHING_ON_0_REASON_TAG_INSIDE_EFFECT_1_HAND_ROLLS_REASON_DISPATCH_USE_EFFECT_CATCH_REASON_OR_EFFECT_CATCH_REASONS_WHICH_RE_FAIL_UNMATCHED_REASONS_AUTOMATICALLY_EFFECT_CATCH_TAG_TO_CATCH_REASON;

// go: internal/fixables/catch_tag_to_catch_reason.go:16 CatchTagToCatchReasonFix 80cdefa5cbdde692
pub static CATCH_TAG_TO_CATCH_REASON_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "catchTagToCatchReason",
    description: "Replace hand-rolled reason dispatch with Effect.catchReason or Effect.catchReasons",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["catchTagToCatchReason_fix"],
    run: run_catch_tag_to_catch_reason_fix,
});

// go: internal/fixables/catch_tag_to_catch_reason.go:26 runCatchTagToCatchReasonFix ee2d21264449304a
//
// Rust: the match's transformation, callee and handler are never nil (non-optional fields), so Go's nil checks
// on them drop. Go's edit closure names each branch's reason parameter with the checker
// (`uniqueCatchReasonParameterName`); the checker is borrowed mutably by the context, which `new_fix_action`
// borrows, so those names (which read only the source and the checker, never the tracker) are computed just before
// `new_fix_action`, in branch order, for the branches that use the parameter.
fn run_catch_tag_to_catch_reason_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    let matches =
        ets_rules::analyze_catch_tag_to_catch_reason(&mut ctx.type_parser, Some(source_file));
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        // Only Effect.catchTag is rewritten. The new handlers rebind the parameter
        // per branch, which only holds for an arrow function whose parameter is
        // never reassigned.
        if m.outer_tag.is_none()
            || m.handler.kind() != Kind::ArrowFunction
            || ctx.checker().is_symbol_assigned(m.parameter_symbol)
        {
            continue;
        }
        let reason_parameter_names: Vec<String> = m
            .branches
            .iter()
            .map(|branch| {
                if branch.uses_parameter {
                    unique_catch_reason_parameter_name(
                        ctx.checker(),
                        branch.result.expect("a branch has a result"),
                    )
                } else {
                    String::new()
                }
            })
            .collect();

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.catchReason or Effect.catchReasons".to_string(),
            run: Box::new(move |tracker| {
                let (callee, arguments) = build_catch_tag_to_catch_reason_replacement(
                    tracker,
                    source_file,
                    &m,
                    &reason_parameter_names,
                );
                if callee.is_none() || arguments.is_none() {
                    return;
                }
                tracker.replace_piping_flow_transformation(
                    Some(source_file),
                    Some(&m.transformation),
                    PipingFlowTransformationReplacement { callee, type_arguments: None, arguments },
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}

// go: internal/fixables/catch_tag_to_catch_reason.go:61 buildCatchTagToCatchReasonReplacement cd07e74d56fa77bb
//
// Rust: the tracker and the match's callee cannot be nil. Go's `ctx` parameter is the source file plus
// `reason_parameter_names`, each branch's precomputed reason parameter name (see
// `run_catch_tag_to_catch_reason_fix`).
fn build_catch_tag_to_catch_reason_replacement(
    tracker: &Tracker,
    sf: P<SourceFile>,
    m: &CatchTagToCatchReasonMatch,
    reason_parameter_names: &[String],
) -> (Option<P<Node>>, Option<P<NodeList>>) {
    let Some(outer_tag) = m.outer_tag else {
        return (None, None);
    };
    if m.branches.is_empty() {
        return (None, None);
    }

    let mut method_name = "catchReason";
    if m.branches.len() > 1 {
        method_name = "catchReasons";
    }
    let method = tracker.new_property_access_path(
        &ets_typeparser::find_effect_module_reference(Some(sf)).append(&[method_name]),
    );
    let mut arguments = vec![tracker.deep_clone_node(Some(outer_tag)).unwrap()];
    if m.branches.len() == 1 {
        let branch = &m.branches[0];
        arguments.push(tracker.new_string_literal(alloc_str(&branch.reason_tag), TokenFlags::None));
        arguments.push(new_catch_reason_handler(
            tracker,
            &reason_parameter_names[0],
            &m.parameter_name,
            branch,
        ));
    } else {
        let mut properties = Vec::with_capacity(m.branches.len());
        for (index, branch) in m.branches.iter().enumerate() {
            properties.push(tracker.new_property_assignment(
                None,
                tracker.new_string_literal(alloc_str(&branch.reason_tag), TokenFlags::None),
                None,
                None,
                new_catch_reason_handler(
                    tracker,
                    &reason_parameter_names[index],
                    &m.parameter_name,
                    branch,
                ),
            ));
        }
        arguments
            .push(tracker.new_object_literal_expression(tracker.new_node_list(properties), false));
    }

    (Some(method), Some(tracker.new_node_list(arguments)))
}

// go: internal/fixables/catch_tag_to_catch_reason.go:91 newCatchReasonHandler 45ba73053516917a
//
// Rust: Go's checker parameter is `reason_parameter_name`, the branch's precomputed
// `uniqueCatchReasonParameterName` result.
fn new_catch_reason_handler(
    tracker: &Tracker,
    reason_parameter_name: &str,
    parameter_name: &str,
    branch: &CatchTagToCatchReasonBranch,
) -> P<Node> {
    let mut parameters = Vec::new();
    if branch.uses_parameter {
        parameters = vec![
            tracker.new_parameter_declaration(
                None,
                None,
                tracker.new_identifier(alloc_str(reason_parameter_name)),
                None,
                None,
                None,
            ),
            tracker.new_parameter_declaration(
                None,
                None,
                tracker.new_identifier(alloc_str(parameter_name)),
                None,
                None,
                None,
            ),
        ];
    }
    tracker.new_arrow_function(
        None,
        None,
        Some(tracker.new_node_list(parameters)),
        None,
        None,
        Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
        tracker.deep_clone_node(branch.result),
    )
}

// go: internal/fixables/catch_tag_to_catch_reason.go:112 uniqueCatchReasonParameterName 321d2e7bdae7bf7d
// uniqueCatchReasonParameterName returns "_", or "_0", "_1", ... when taken,
// for the unused reason parameter that precedes the error parameter.
fn unique_catch_reason_parameter_name(c: &mut Checker, location: P<Node>) -> String {
    let mut used: FxHashSet<String> = FxHashSet::default();
    for symbol in c.get_symbols_in_scope_exported(location, SymbolFlags::Value) {
        used.insert(c.symbol_to_string_exported(symbol));
    }
    let mut i: i64 = -1;
    loop {
        let mut name = "_".to_string();
        if i >= 0 {
            name = format!("_{i}");
        }
        if !used.contains(&name) {
            return name;
        }
        i += 1;
    }
}
