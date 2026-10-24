use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::PipingFlowTransformationReplacement;
use ets_shim::ast::{self, Kind, NodeFlags};
use ets_shim::core::alloc_str;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message =
    &tsdiag::Use_Effect_0_to_handle_this_timeout_directly_effect_timeoutCatchTagToTimeoutOrElse;

// go: internal/fixables/timeout_catch_tag_to_timeout_or_else.go:13 TimeoutCatchTagToTimeoutOrElseFix
pub static TIMEOUT_CATCH_TAG_TO_TIMEOUT_OR_ELSE_FIX: LazyLock<Fixable> =
    LazyLock::new(|| Fixable {
        name: "timeoutCatchTagToTimeoutOrElse",
        description: "Replace with a dedicated timeout combinator",
        error_codes: vec![MESSAGE.code()],
        fix_ids: vec!["timeoutCatchTagToTimeoutOrElse_fix"],
        run: |ctx: &mut Context<'_, '_>| -> Vec<CodeAction> {
            let sf = ctx.source_file;
            for m in ets_rules::analyze_timeout_catch_tag_to_timeout_or_else(
                &mut ctx.type_parser,
                Some(sf),
            ) {
                if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
                    continue;
                }
                let action = ctx.new_fix_action(FixAction {
                    description: "Replace with Effect.".to_string() + m.replacement_name,
                    run: Box::new(move |t| {
                        let effect_module = match m.effect_module {
                            Some(effect_module) => t.deep_clone_node(Some(effect_module)).unwrap(),
                            None => t.new_identifier(alloc_str(
                                &ets_typeparser::find_effect_module_identifier(sf),
                            )),
                        };
                        let callee = t.new_property_access_expression(
                            effect_module,
                            None,
                            t.new_identifier(m.replacement_name),
                            NodeFlags::None,
                        );
                        let mut argument = t.deep_clone_node(Some(m.duration)).unwrap();
                        if m.replacement_name == "timeoutOrElse" {
                            let mut handler = t.deep_clone_node(Some(m.handler)).unwrap();
                            let arrow = ast::skip_parentheses(m.handler).as_arrow_function();
                            if arrow
                                .parameters()
                                .is_some_and(|parameters| !parameters.nodes().is_empty())
                            {
                                handler = t.new_arrow_function(
                                    None,
                                    None,
                                    Some(t.new_node_list(Vec::new())),
                                    t.deep_clone_node(arrow.type_()),
                                    None,
                                    Some(t.new_token(Kind::EqualsGreaterThanToken)),
                                    t.deep_clone_node(arrow.body()),
                                );
                            }
                            argument = t.new_object_literal_expression(
                                t.new_node_list(vec![
                                    t.new_property_assignment(
                                        None,
                                        t.new_identifier("duration"),
                                        None,
                                        None,
                                        argument,
                                    ),
                                    t.new_property_assignment(
                                        None,
                                        t.new_identifier("orElse"),
                                        None,
                                        None,
                                        handler,
                                    ),
                                ]),
                                true,
                            );
                        }
                        let arguments = t.new_node_list(vec![argument]);
                        t.collapse_piping_flow_transformations(
                            Some(sf),
                            Some(&m.flow),
                            m.start,
                            m.end,
                            PipingFlowTransformationReplacement {
                                callee: Some(callee),
                                arguments: Some(arguments),
                                ..Default::default()
                            },
                        );
                    }),
                });
                if let Some(action) = action {
                    return vec![action];
                }
            }
            Vec::new()
        },
    });
