use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::SCHEMA_LITERAL_VALUES_MUST_BE_FINITE_NUMBERS_0_THROWS_DURING_SCHEMA_CONSTRUCTION_USE_A_FINITE_LITERAL_OR_MODEL_NON_FINITE_VALUES_WITH_A_REFINED_SCHEMA_NUMBER_EFFECT_SCHEMA_LITERAL_NON_FINITE;

// go: internal/rules/schema_literal_non_finite.go:13 SchemaLiteralNonFinite d81cc2eb6ea7c45c
pub static SCHEMA_LITERAL_NON_FINITE: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "schemaLiteralNonFinite",
    group: "correctness",
    description: "Reports statically known non-finite numbers passed to Schema literal constructors",
    default_severity: Severity::Error,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(ctx: &mut Context<'_, '_>, node: P<Node>, diags: &mut Vec<P<Diagnostic>>) -> bool {
            if node.kind() == Kind::CallExpression {
                let call = node.as_call_expression();
                let arguments = call.arguments().nodes();
                if !arguments.is_empty() {
                    let mut values: &[P<Node>] = &[];
                    let expression = call.expression();
                    if ctx
                        .type_parser
                        .is_node_reference_to_effect_schema_module_api(expression, "Literals")
                    {
                        let arg = arguments[0];
                        if arg.kind() == Kind::ArrayLiteralExpression {
                            values = arg.as_array_literal_expression().elements().nodes();
                        }
                    } else if ctx
                        .type_parser
                        .is_node_reference_to_effect_schema_module_api(expression, "Literal")
                        || ctx
                            .type_parser
                            .is_node_reference_to_effect_schema_module_api(expression, "tag")
                    {
                        values = &arguments[..1];
                    }

                    for &value in values {
                        if let Some(n) =
                            ctx.type_parser.evaluate_constant_number(value, Some(value))
                            && (n.is_infinite() || n.is_nan())
                        {
                            diags.push(ctx.new_diagnostic(
                                ctx.source_file,
                                ets_shim::scanner::get_error_range_for_node(ctx.source_file, value),
                                MESSAGE,
                                &[],
                                &[&ets_shim::scanner::get_text_of_node(value)],
                            ));
                        }
                    }
                }
            }

            node.for_each_child(&mut |child| walk(ctx, child, diags));
            false
        }

        let root = ctx.source_file.as_node();
        walk(ctx, root, &mut diags);
        diags
    },
});
