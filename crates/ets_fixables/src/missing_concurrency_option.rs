use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::{PipingFlowTransformationReplacement, Tracker};
use ets_rules::MissingConcurrencyOptionMatch;
use ets_shim::P;
use ets_shim::ast::{self, Node, NodeFactory, NodeFlags, NodeList, SourceFile, TokenFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::{PipingFlow, TransformationKind};

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_0_RUNS_SEQUENTIALLY_UNLESS_CONCURRENCY_IS_SET_PASS_CONCURRENCY_COLON_1_TO_KEEP_SEQUENTIAL_EXECUTION_OR_SET_THE_CONCURRENCY_YOU_INTEND_EFFECT_MISSING_CONCURRENCY_OPTION;

// go: internal/fixables/missing_concurrency_option.go:13 MissingConcurrencyOptionFix c16d3c62a957e3b3
pub static MISSING_CONCURRENCY_OPTION_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "missingConcurrencyOption",
    description: "Add `concurrency: 1` to keep the sequential behaviour explicit",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["missingConcurrencyOption_fix"],
    run: run_missing_concurrency_option_fix,
});

// go: internal/fixables/missing_concurrency_option.go:23 runMissingConcurrencyOptionFix 6e83041f4f095a5c
fn run_missing_concurrency_option_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_missing_concurrency_option(&mut ctx.type_parser, sf) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Add `concurrency: 1`".to_string(),
            run: Box::new(move |tracker| {
                add_sequential_concurrency(tracker, sf, &m);
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}

// go: internal/fixables/missing_concurrency_option.go:41 newSequentialConcurrencyProperty 1a55dc675ddd38bf
fn new_sequential_concurrency_property(tracker: &NodeFactory) -> P<Node> {
    tracker.new_property_assignment(
        None,
        tracker.new_identifier("concurrency"),
        None,
        None,
        tracker.new_numeric_literal("1", TokenFlags::None),
    )
}

// go: internal/fixables/missing_concurrency_option.go:45 newSequentialConcurrencyOptions d31a4cff7966b000
fn new_sequential_concurrency_options(tracker: &NodeFactory) -> P<Node> {
    tracker.new_object_literal_expression(
        tracker.new_node_list(vec![new_sequential_concurrency_property(tracker)]),
        false,
    )
}

// go: internal/fixables/missing_concurrency_option.go:49 addSequentialConcurrency 41b9fd78de4a5c90
fn add_sequential_concurrency(
    tracker: &mut Tracker,
    sf: P<SourceFile>,
    m: &MissingConcurrencyOptionMatch,
) {
    if let Some(options_literal) = m.options_literal {
        add_sequential_concurrency_to_literal(tracker, sf, options_literal);
        return;
    }

    let step = m.flow.transformations().at(m.step_index);
    if ets_rules::is_input_only_call(step) {
        let Some(input) = PipingFlow::transformation_input_node(Some(&m.flow), m.step_index) else {
            return;
        };
        let callee = tracker.deep_clone_node(Some(step.callee())).unwrap();
        let type_arguments = clone_type_arguments(tracker, step.type_arguments());
        let arguments = vec![
            tracker.deep_clone_node(Some(input)).unwrap(),
            new_sequential_concurrency_options(tracker),
        ];
        let replacement = tracker.new_call_expression(
            callee,
            None,
            type_arguments,
            tracker.new_node_list(arguments),
            NodeFlags::None,
        );
        tracker.replace_piping_flow_prefix(
            Some(sf),
            Some(&m.flow),
            m.step_index + 1,
            Some(replacement),
        );
    } else if step.kind() == TransformationKind::DataFirst
        || step.kind() == TransformationKind::Pipe
        || step.kind() == TransformationKind::Pipeable
        || step.kind() == TransformationKind::Call
    {
        let mut arguments = Vec::with_capacity(step.args().len() as usize + 1);
        for (_, &argument) in step.args().all() {
            arguments.push(tracker.deep_clone_node(Some(argument)).unwrap());
        }
        arguments.push(new_sequential_concurrency_options(tracker));
        let replacement = PipingFlowTransformationReplacement {
            callee: tracker.deep_clone_node(Some(step.callee())),
            type_arguments: clone_type_arguments(tracker, step.type_arguments()),
            arguments: Some(tracker.new_node_list(arguments)),
        };
        tracker.replace_piping_flow_transformation(Some(sf), Some(step), replacement);
    }
}

// go: internal/fixables/missing_concurrency_option.go:87 addSequentialConcurrencyToLiteral c18c0c7b5889a53c
// Rust: tsrs's object literal properties are never nil, so Go's nil check drops.
fn add_sequential_concurrency_to_literal(
    tracker: &mut Tracker,
    sf: P<SourceFile>,
    literal: P<Node>,
) {
    let properties = literal.as_object_literal_expression().properties;
    if properties.nodes().is_empty() {
        let replacement = new_sequential_concurrency_options(tracker);
        ast::set_parent_in_children(replacement);
        tracker.replace_node(sf, Some(literal), Some(replacement), None);
        return;
    }
    let after = properties.nodes()[properties.nodes().len() - 1];
    let new_node = new_sequential_concurrency_property(tracker);
    tracker.insert_node_in_list_after(sf, after, new_node, Some(properties));
}

// go: internal/fixables/missing_concurrency_option.go:98 cloneTypeArguments 9705f21a2638bce8
fn clone_type_arguments(
    tracker: &NodeFactory,
    type_arguments: Option<P<NodeList>>,
) -> Option<P<NodeList>> {
    let type_arguments = type_arguments?;
    let mut nodes = Vec::with_capacity(type_arguments.nodes().len());
    for &node in type_arguments.nodes() {
        nodes.push(tracker.deep_clone_node(Some(node)).unwrap());
    }
    Some(tracker.new_node_list(nodes))
}
