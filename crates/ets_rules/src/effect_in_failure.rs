// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};
use rustc_hash::FxHashSet;

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THE_ERROR_CHANNEL_CONTAINS_AN_EFFECT_0_PUTTING_EFFECT_COMPUTATIONS_IN_THE_FAILURE_CHANNEL_IS_NOT_INTENDED_KEEP_ONLY_FAILURE_TYPES_THERE_EFFECT_EFFECT_IN_FAILURE;

// go: internal/rules/effect_in_failure.go:15 EffectInFailure 1f3d7b9610e8f8e8
// EffectInFailure detects when an Effect type appears in the failure (E) channel
// of another Effect. Putting Effect computations in the failure channel is not
// intended; only failure types should appear there.
pub static EFFECT_IN_FAILURE: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "effectInFailure",
    group: "antipattern",
    description: "Warns when an Effect is used inside an Effect failure channel",
    default_severity: Severity::Warning,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();

        // Post-order AST walk using a stack with a visited set.
        // Children are processed before parents so that the
        // shouldSkipBecauseChildMatched mechanism can suppress
        // redundant diagnostics on parent nodes.
        struct StackEntry {
            node: P<Node>,
            visited: bool,
        }

        let mut stack: Vec<StackEntry> =
            vec![StackEntry { node: ctx.source_file.as_node(), visited: false }];
        let mut should_skip: FxHashSet<P<Node>> = FxHashSet::default();

        while let Some(entry) = stack.pop() {
            let node = entry.node;

            // First visit: push self again (marked visited) then push children
            if !entry.visited {
                stack.push(StackEntry { node, visited: true });
                node.for_each_child(&mut |child| {
                    stack.push(StackEntry { node: child, visited: false });
                    false
                });
                continue;
            }

            // Second visit (post-order): check the node

            // If a child already matched, propagate skip to parent and continue
            if should_skip.contains(&node) {
                if let Some(parent) = node.parent() {
                    should_skip.insert(parent);
                }
                continue;
            }

            if ets_typeparser::type_equivalent_node(node).1 {
                continue;
            }

            // Declared-type prefilter: skip the expensive flow-analysis query
            // for reference nodes that conclusively cannot have a strict
            // Effect flow type. Skipped nodes can never match, so no
            // shouldSkip bookkeeping is needed.
            if !ctx.type_parser.node_could_be_strict_effect(node) {
                continue;
            }

            let Some(node_type) = ctx.type_parser.get_type_at_location(node) else {
                continue;
            };

            let Some(effect) = ctx.type_parser.strict_effect_type(node_type) else {
                continue;
            };

            // Check if any union member of the failure channel (E) is a strict Effect type
            let failure_members =
                ctx.type_parser.unroll_union_members(effect.e()).unwrap_or_default();
            let Some(member_with_effect) =
                find_first_strict_effect(ctx.type_parser, &failure_members)
            else {
                continue;
            };

            let type_string = ctx.checker().type_to_string_exported(member_with_effect);
            let diag = ctx.new_diagnostic(
                ctx.source_file,
                ctx.get_error_range(node),
                MESSAGE,
                &[],
                &[&type_string],
            );
            diags.push(diag);

            // Mark parent to skip redundant reporting
            if let Some(parent) = node.parent() {
                should_skip.insert(parent);
            }
        }

        diags
    },
});
