use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Node};
use ets_shim::checker::TypeFlags;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THE_PREVIOUS_EFFECT_DOES_NOT_FAIL_SO_THIS_ERROR_HANDLING_BRANCH_WILL_NEVER_RUN_EFFECT_CATCH_UNFAILABLE_EFFECT;

// go: internal/rules/catch_unfailable_effect.go:14 catchFunctions ee141a557cda15b7
// catchFunctions lists the Effect module catch functions to check (V3 and V4).
pub(crate) static CATCH_FUNCTIONS: &[&str] =
    &["catch", "catchAll", "catchIf", "catchSome", "catchTag", "catchTags"];

// go: internal/rules/catch_unfailable_effect.go:18 CatchUnfailableEffect 31314a465e637468
// CatchUnfailableEffect detects when error-handling functions are applied
// to an Effect whose error type is never, meaning the handler will never trigger.
pub static CATCH_UNFAILABLE_EFFECT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "catchUnfailableEffect",
    group: "antipattern",
    description: "Warns when using error handling on Effects that never fail",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();

        let source_file = ctx.source_file;
        let flows = ctx.type_parser.piping_flows(source_file, true);
        for (_, flow) in ets_readonly::new(flows.as_deref()).all() {
            for (i, transformation) in flow.transformations().all() {
                if !is_catch_callee(ctx.type_parser, transformation.callee()) {
                    continue;
                }

                // Determine the input type for this transformation
                let input_type = if i == 0 {
                    flow.subject().out_type(ctx.type_parser)
                } else {
                    flow.transformations().at(i - 1).out_type(ctx.type_parser)
                };
                let Some(input_type) = input_type else {
                    continue;
                };

                // Parse input type as an Effect
                let Some(effect) = ctx.type_parser.effect_type(input_type) else {
                    continue;
                };

                // Check if E is never
                if !effect.e().flags().intersects(TypeFlags::Never) {
                    continue;
                }

                diags.push(
                    ctx.new_diagnostic(
                        ast::get_source_file_of_node(transformation.callee())
                            .expect("transformation callee is in a source file"),
                        ctx.get_error_range(transformation.callee()),
                        MESSAGE,
                        &[],
                        &[],
                    ),
                );
            }
        }

        diags
    },
});

// go: internal/rules/catch_unfailable_effect.go:66 isCatchCallee 14a58a3a4bed3a57
// isCatchCallee checks if a node references one of the Effect module catch functions.
//
// Rust: Go's checker argument is unused and dropped.
pub(crate) fn is_catch_callee(tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
    for name in CATCH_FUNCTIONS {
        if tp.is_node_reference_to_effect_module_api(node, name) {
            return true;
        }
    }
    false
}
