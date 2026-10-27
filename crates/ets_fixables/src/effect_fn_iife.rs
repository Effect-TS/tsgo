use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rules::EffectFnIifeResult;
use ets_shim::ast::{self, NodeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::X_0_1_RETURNS_A_REUSABLE_FUNCTION_THAT_CAN_TAKE_ARGUMENTS_BUT_IT_IS_INVOKED_IMMEDIATELY_HERE_EFFECT_GEN_REPRESENTS_THE_IMMEDIATE_USE_FORM_FOR_THIS_PATTERN_2_EFFECT_EFFECT_FN_IIFE;

// go: internal/fixables/effect_fn_iife.go:12 EffectFnIifeFix
pub static EFFECT_FN_IIFE_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "effectFnIife",
    description: "Convert Effect.fn IIFE to Effect.gen",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["effectFnIife_toEffectGen"],
    run: run_effect_fn_iife_fix,
});

// go: internal/fixables/effect_fn_iife.go:20 runEffectFnIifeFix
fn run_effect_fn_iife_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    // Use shared analysis to find all IIFE matches, then find the one at the diagnostic span
    let matches = ets_rules::analyze_effect_fn_iife(&mut ctx.type_parser, sf);
    let mut result: Option<EffectFnIifeResult> = None;
    for m in matches {
        let diag_range = m.location;
        if diag_range.intersects(ctx.span) || ctx.span.contained_by(diag_range) {
            result = Some(m.result);
            break;
        }
    }
    let Some(result) = result else {
        return Vec::new();
    };

    // Only produce a fix for generator functions with zero parameters
    let Some(gen_fn) = result.generator_function else {
        return Vec::new();
    };
    if let Some(parameters) = gen_fn.as_function_expression().parameters()
        && !parameters.nodes().is_empty()
    {
        return Vec::new();
    }

    if let Some(action) = ctx.new_fix_action(FixAction {
        description: "Convert to Effect.gen".to_string(),
        run: Box::new(move |tracker| {
            // Deep-clone the generator function to produce a synthesized copy
            let cloned_gen_fn = tracker.deep_clone_node(Some(gen_fn)).unwrap();

            // Build Effect.gen from the file's Effect import.
            let effect_gen_access = effect_module_method(tracker, sf, "gen");

            // Build Effect.gen(genFn) call expression
            let gen_call_node = tracker.new_call_expression(
                effect_gen_access,
                None,
                None,
                tracker.new_node_list(vec![cloned_gen_fn]),
                NodeFlags::None,
            );

            // If pipe arguments exist, wrap with .pipe(...)
            let mut replacement_node = gen_call_node;
            let pipe_arguments = result.pipe_arguments.as_deref().unwrap_or_default();
            if !pipe_arguments.is_empty() {
                let mut cloned_args = Vec::with_capacity(pipe_arguments.len());
                for &arg in pipe_arguments {
                    cloned_args.push(tracker.deep_clone_node(Some(arg)).unwrap());
                }
                let pipe_access = tracker.new_property_access_expression(
                    gen_call_node,
                    None,
                    tracker.new_identifier("pipe"),
                    NodeFlags::None,
                );
                replacement_node = tracker.new_call_expression(
                    pipe_access,
                    None,
                    None,
                    tracker.new_node_list(cloned_args),
                    NodeFlags::None,
                );
            }

            // Set parent pointers on the entire replacement tree so the
            // formatter's token-walking code doesn't panic on nil Parent
            ast::set_parent_in_children(replacement_node);

            tracker.replace_node(sf, Some(result.outer_call), Some(replacement_node), None);
        }),
    }) {
        return vec![action];
    }
    Vec::new()
}
