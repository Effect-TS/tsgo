use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::P;
use ets_shim::ast::{Kind, Node, NodeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_shim::scanner;
use ets_typeparser::{TransformationKind, TypeParser};

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_0_IS_CALLED_WITHOUT_A_CATCH_HANDLER_SO_FAILURES_SURFACE_AS_THE_GENERIC_1_THE_OBJECT_FORM_TRY_CATCH_MAPS_THE_CAUSE_TO_A_TYPED_DOMAIN_SPECIFIC_ERROR_EFFECT_EFFECT_TRY_WITHOUT_CATCH;

// go: internal/fixables/effect_try_without_catch.go:15 EffectTryWithoutCatchFix 03764ec0f6d99cee
pub static EFFECT_TRY_WITHOUT_CATCH_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "effectTryWithoutCatch",
    description: "Add a catch handler (object form)",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["effectTryWithoutCatch_fix"],
    run: run_effect_try_without_catch_fix,
});

// go: internal/fixables/effect_try_without_catch.go:25 runEffectTryWithoutCatchFix 1ee80b0492ee9585
fn run_effect_try_without_catch_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_effect_try_without_catch(&mut ctx.type_parser, Some(sf)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        // The thunk moves into the options object, so it must be a standalone
        // node: a later pipe step has no node for its input.
        let Some(thunk) = m.thunk else {
            continue;
        };
        let step = m.step().clone();
        // The short form takes <A>, the object form <A, E>: a single written type
        // argument gets an `unknown` error type, while two cannot be the short form.
        let type_arguments = step.type_arguments();
        if type_arguments.is_some_and(|type_arguments| type_arguments.nodes().len() > 1) {
            continue;
        }
        if !callee_has_effect_api_type(&mut ctx.type_parser, step.callee()) {
            continue;
        }

        let ctx_ref: &Context<'_, '_> = ctx;
        if let Some(action) = ctx_ref.new_fix_action(FixAction {
            description: "Add a catch handler (object form)".to_string(),
            run: Box::new(move |tracker| {
                if step.kind() == TransformationKind::Call {
                    // The thunk is already the call's argument: wrapping it in place keeps
                    // its text and comments, which the flow edits would reprint.
                    if let Some(type_arguments) = type_arguments
                        && type_arguments.nodes().len() == 1
                    {
                        tracker.insert_text(
                            sf,
                            ctx_ref.byte_pos_to_lsp_position(
                                type_arguments.nodes()[0]
                                    .end()
                                    .expect_source("a parsed type argument"),
                            ),
                            ", unknown",
                        );
                    }
                    tracker.insert_text(
                        sf,
                        ctx_ref.byte_pos_to_lsp_position(scanner::get_token_pos_of_node(
                            thunk, sf, false,
                        )),
                        "{ try: ",
                    );
                    tracker.insert_text(
                        sf,
                        ctx_ref
                            .byte_pos_to_lsp_position(thunk.end().expect_source("a parsed thunk")),
                        ", catch: (cause) => cause }",
                    );
                    return;
                }

                // A pipe step applies the API to its subject, so the subject and the
                // step become one object-form call, keeping the later steps.
                let mut type_argument_list = None;
                if let Some(type_arguments) = type_arguments
                    && type_arguments.nodes().len() == 1
                {
                    type_argument_list = Some(tracker.new_node_list(vec![
                        tracker.deep_clone_node(Some(type_arguments.nodes()[0])).unwrap(),
                        tracker.new_keyword_type_node(Kind::UnknownKeyword),
                    ]));
                }
                let cause = tracker.new_identifier("cause");
                let catch_handler = tracker.new_arrow_function(
                    None,
                    None,
                    Some(tracker.new_node_list(vec![
                        tracker.new_parameter_declaration(None, None, cause, None, None, None),
                    ])),
                    None,
                    None,
                    Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
                    Some(tracker.new_identifier("cause")),
                );
                let options = tracker.new_object_literal_expression(
                    tracker.new_node_list(vec![
                        tracker.new_property_assignment(
                            None,
                            tracker.new_identifier("try"),
                            None,
                            None,
                            tracker.deep_clone_node(Some(thunk)).unwrap(),
                        ),
                        tracker.new_property_assignment(
                            None,
                            tracker.new_identifier("catch"),
                            None,
                            None,
                            catch_handler,
                        ),
                    ]),
                    false,
                );
                let replacement = tracker.new_call_expression(
                    tracker.deep_clone_node(Some(step.callee())).unwrap(),
                    None,
                    type_argument_list,
                    tracker.new_node_list(vec![options]),
                    NodeFlags::None,
                );
                tracker.replace_piping_flow_prefix(
                    Some(sf),
                    Some(&m.flow),
                    m.index + 1,
                    Some(replacement),
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}

// go: internal/fixables/effect_try_without_catch.go:102 calleeHasEffectApiType 7b3c6f7d34814ac0
// calleeHasEffectApiType reports whether callee still has the declared type of
// the Effect API it resolves to, and so its object-form overload. Const aliases
// resolve to the API whatever their annotation, which may drop that overload.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
fn callee_has_effect_api_type(tp: &mut TypeParser<'_>, callee: P<Node>) -> bool {
    let symbol = tp.reference_symbol_at_node(callee);
    let callee_type = tp.get_type_at_location(callee);
    match (symbol, callee_type) {
        (Some(symbol), Some(callee_type)) => {
            tp.checker().get_type_of_symbol_exported(symbol) == callee_type
        }
        _ => false,
    }
}
