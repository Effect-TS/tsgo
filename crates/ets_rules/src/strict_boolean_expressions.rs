use std::collections::VecDeque;
use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::checker::{Type, TypeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use rustc_hash::FxHashSet;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::UNEXPECTED_0_TYPE_IN_CONDITION_EXPECTED_STRICTLY_A_BOOLEAN_INSTEAD_EFFECT_STRICT_BOOLEAN_EXPRESSIONS;

// go: internal/rules/strict_boolean_expressions.go:13 StrictBooleanExpressions 40afab6b12c9e9fa
// StrictBooleanExpressions enforces that expressions in conditional positions
// are strictly boolean-typed, reporting non-boolean types as diagnostics.
pub static STRICT_BOOLEAN_EXPRESSIONS: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "strictBooleanExpressions",
    group: "style",
    description: "Enforces boolean types in conditional expressions for type safety",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();
        // Rust: Go's `map[*ast.Node]bool` only ever stores true, so it is a set.
        let mut condition_checks: FxHashSet<P<Node>> = FxHashSet::default();
        let is_condition_check = |checks: &FxHashSet<P<Node>>, node: Option<P<Node>>| {
            node.is_some_and(|node| checks.contains(&node))
        };

        // Breadth-first traversal matching the reference implementation
        let mut node_to_visit: VecDeque<P<Node>> = VecDeque::new();
        ctx.source_file.as_node().for_each_child(&mut |child| {
            node_to_visit.push_back(child);
            false
        });

        while let Some(node) = node_to_visit.pop_front() {
            // Enqueue children
            node.for_each_child(&mut |child| {
                node_to_visit.push_back(child);
                false
            });

            let mut nodes_to_check: Vec<P<Node>> = Vec::new();

            match node.kind() {
                Kind::IfStatement => {
                    condition_checks.insert(node);
                    nodes_to_check.push(node.as_if_statement().expression());
                }

                Kind::WhileStatement => {
                    condition_checks.insert(node);
                    nodes_to_check.push(node.as_while_statement().expression());
                }

                Kind::ConditionalExpression => {
                    condition_checks.insert(node);
                    nodes_to_check.push(node.as_conditional_expression().condition());
                }

                Kind::PrefixUnaryExpression => {
                    let prefix = node.as_prefix_unary_expression();
                    if prefix.operator() == Kind::ExclamationToken {
                        condition_checks.insert(node);
                        nodes_to_check.push(prefix.operand());
                    }
                }

                Kind::BinaryExpression => {
                    let bin_expr = node.as_binary_expression();
                    let op_kind = bin_expr.operator_token().kind();
                    if op_kind == Kind::BarBarToken || op_kind == Kind::AmpersandAmpersandToken {
                        if is_condition_check(&condition_checks, node.parent()) {
                            condition_checks.insert(node);
                        }
                        nodes_to_check.push(bin_expr.left());
                        nodes_to_check.push(bin_expr.right());
                    }
                }

                _ => {}
            }

            for node_to_check in nodes_to_check {
                if !is_condition_check(&condition_checks, node_to_check.parent()) {
                    continue;
                }

                let Some(node_type) = ctx.type_parser.get_type_at_location(node_to_check) else {
                    continue;
                };

                let constrained_type = ctx.checker().get_base_constraint_of_type(node_type);
                let mut types_to_check: Vec<P<Type>> = Vec::new();
                if let Some(constrained_type) = constrained_type {
                    types_to_check.push(constrained_type);
                } else {
                    types_to_check.push(node_type);
                }
                let mut seen_type_names: FxHashSet<String> = FxHashSet::default();

                while let Some(t) = types_to_check.pop() {
                    // Unroll union types
                    if t.flags().intersects(TypeFlags::Union) {
                        types_to_check
                            .extend(ctx.type_parser.unroll_union_members(t).unwrap_or_default());
                        continue;
                    }

                    // Skip boolean and never types
                    if t.flags().intersects(TypeFlags::Boolean) {
                        continue;
                    }
                    if t.flags().intersects(TypeFlags::BooleanLiteral) {
                        continue;
                    }
                    if t.flags().intersects(TypeFlags::Never) {
                        continue;
                    }

                    let type_name = ctx.checker().type_to_string_exported(t);
                    if seen_type_names.contains(&type_name) {
                        continue;
                    }
                    seen_type_names.insert(type_name.clone());

                    // Report the error
                    diags.push(ctx.new_diagnostic(
                        ctx.source_file,
                        ctx.get_error_range(node_to_check),
                        MESSAGE,
                        &[],
                        &[&type_name],
                    ));
                }
            }
        }

        diags
    },
});
