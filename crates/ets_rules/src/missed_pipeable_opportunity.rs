use std::sync::LazyLock;

use ets_etscore::{ResolvedEffectPluginOptions, Severity};
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Node, SourceFile};
use ets_shim::checker::SignatureKind;
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{PipingFlow, PipingFlowTransformation, TransformationKind, TypeParser};

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_NESTED_CALL_STRUCTURE_HAS_A_PIPEABLE_FORM_0_PIPE_REPRESENTS_THE_SAME_CALL_SEQUENCE_IN_PIPE_STYLE_AND_MAY_BE_EASIER_TO_READ_EFFECT_MISSED_PIPEABLE_OPPORTUNITY;

// go: internal/rules/missed_pipeable_opportunity.go:19 MissedPipeableOpportunity d96dffe723cf29e9
// MissedPipeableOpportunity detects nested function call chains that can be converted to .pipe() style.
pub static MISSED_PIPEABLE_OPPORTUNITY: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "missedPipeableOpportunity",
    group: "style",
    description: "Suggests using .pipe() for nested function calls",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut min_arg_count = 2;
        let effect_config = ctx.options;
        if effect_config.is_some() {
            min_arg_count = ResolvedEffectPluginOptions::get_pipeable_min_arg_count(effect_config);
        }

        let matches =
            analyze_missed_pipeable_opportunity(ctx.type_parser, ctx.source_file, min_arg_count);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[&m.subject_text],
            ));
        }
        diags
    },
});

// go: internal/rules/missed_pipeable_opportunity.go:50 MissedPipeableOpportunityMatch b4984fecfc72d98f
// MissedPipeableOpportunityMatch holds the parsed result needed by both the diagnostic rule
// and the quick-fix for the missedPipeableOpportunity pattern.
//
// Rust: `flow` is a copy of the flow (Go's pointer into the cached flows), and `after_transformations` owns the
// transformations Go's read-only view points at (`None` for Go's nil view).
#[derive(Clone)]
pub struct MissedPipeableOpportunityMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub pipeable_start_index: i64,
    pub pipeable_transformations: Vec<PipingFlowTransformation>,
    pub flow: PipingFlow,
    pub node: P<Node>, // The expression replaced by the fix; may be inner to Flow.Node()
    pub subject_text: String,
    pub after_transformations: Option<Vec<PipingFlowTransformation>>,
}

// go: internal/rules/missed_pipeable_opportunity.go:62 AnalyzeMissedPipeableOpportunity 5e4a6184ad3ab7fe
// AnalyzeMissedPipeableOpportunity finds all nested call chains that can be converted to .pipe() style.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub fn analyze_missed_pipeable_opportunity(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    min_arg_count: i64,
) -> Vec<MissedPipeableOpportunityMatch> {
    let flows = tp.piping_flows(sf, false);

    let mut matches = Vec::new();

    for flow in flows.as_deref().unwrap_or_default() {
        // Skip flows with too few transformations
        if flow.transformations().len() < min_arg_count {
            continue;
        }

        let (reported_count, reported_node) = reportable_transformation_count(tp, flow);
        let Some(reported_node) = reported_node else {
            continue;
        };

        let runs = safely_pipeable_runs(tp, flow);
        for first_pipeable_index in 0..=reported_count {
            let run = runs[first_pipeable_index as usize];
            if run.call_count < min_arg_count
                || !is_pipeable_at_index(tp, flow, first_pipeable_index)
            {
                continue;
            }
            let mut pipeable_transformations = Vec::new();
            for i in first_pipeable_index..first_pipeable_index + run.length {
                pipeable_transformations.push(flow.transformations().at(i).clone());
            }
            let pipeable_end_index = first_pipeable_index + pipeable_transformations.len() as i64;

            let subject_text = trimmed_node_text(
                Some(sf),
                PipingFlow::transformation_input_node(Some(flow), first_pipeable_index),
            );

            let after_transformations =
                flow.transformations().slice(pipeable_end_index, reported_count).copy();

            matches.push(MissedPipeableOpportunityMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, reported_node),
                node: reported_node,
                pipeable_start_index: first_pipeable_index,
                pipeable_transformations,
                flow: flow.clone(),
                subject_text,
                after_transformations,
            });

            // Found and reported a valid segment, move past it (no overlapping diagnostics)
            break;
        }
    }

    matches
}

// go: internal/rules/missed_pipeable_opportunity.go:113 safelyPipeableRun 8da24950a3b99dec
#[derive(Clone, Copy, Default)]
pub(crate) struct SafelyPipeableRun {
    pub(crate) length: i64,
    pub(crate) call_count: i64,
}

// go: internal/rules/missed_pipeable_opportunity.go:118 safelyPipeableRuns b3cfca200a4bd664
pub(crate) fn safely_pipeable_runs(
    tp: &mut TypeParser<'_>,
    flow: &PipingFlow,
) -> Vec<SafelyPipeableRun> {
    let transformation_count = flow.transformations().len();
    let mut runs = vec![SafelyPipeableRun::default(); transformation_count as usize + 1];
    let mut i = transformation_count - 1;
    while i >= 0 {
        let t = flow.transformations().at(i);
        if t.kind() == TransformationKind::CallWithOptionals
            || !tp.is_safely_pipeable_callee(t.callee())
        {
            i -= 1;
            continue;
        }
        let next = runs[i as usize + 1];
        runs[i as usize] =
            SafelyPipeableRun { length: next.length + 1, call_count: next.call_count };
        if t.kind() == TransformationKind::Call {
            runs[i as usize].call_count += 1;
        }
        i -= 1;
    }
    runs
}

// go: internal/rules/missed_pipeable_opportunity.go:136 isPipeableAtIndex 933d04bdf025ee66
// isPipeableAtIndex checks if the type at a given index in a flow is pipeable.
// Index 0 = subject, index > 0 = transformations[index - 1].outType
pub(crate) fn is_pipeable_at_index(tp: &mut TypeParser<'_>, flow: &PipingFlow, index: i64) -> bool {
    if index == 0 {
        if tp.literal_cannot_be_pipeable(flow.subject().node()) {
            return false;
        }
        let Some(subject_type) = flow.subject().out_type(tp) else {
            return false;
        };
        return tp.is_pipeable_type(subject_type);
    }

    let t = flow.transformations().at(index - 1);
    let Some(out_type) = t.out_type(tp) else {
        return false;
    };
    tp.is_pipeable_type(out_type)
}

// go: internal/rules/missed_pipeable_opportunity.go:160 reportableTransformationCount 9db8b89b91246414
// reportableTransformationCount returns how many leading transformations of
// the flow may be reported, and the expression they produce. Flows whose
// output is callable are not reported; when that output comes after a
// callWithOptionals step, the flow is cut before that step instead, as it
// cannot join a pipe anyway. It returns a nil node when nothing is reportable.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn reportable_transformation_count(
    tp: &mut TypeParser<'_>,
    flow: &PipingFlow,
) -> (i64, Option<P<Node>>) {
    let mut count = flow.transformations().len();
    let mut node = flow.node();
    while count > 0 {
        let out_type = flow.transformations().at(count - 1).out_type(tp);
        if out_type.is_none_or(|out_type| {
            tp.checker().get_signatures_of_type(out_type, SignatureKind::Call).is_empty()
        }) {
            return (count, Some(node));
        }
        count -= 1;
        while count >= 0
            && flow.transformations().at(count).kind() != TransformationKind::CallWithOptionals
        {
            count -= 1;
        }
        if count < 0 {
            return (0, None);
        }
        let Some(input) = PipingFlow::transformation_input_node(Some(flow), count) else {
            return (0, None);
        };
        node = input;
    }
    (0, None)
}

// go: internal/rules/missed_pipeable_opportunity.go:184 trimmedNodeText fbfa3a4a1c8bc526
// trimmedNodeText extracts the source text of a node, trimmed of leading trivia.
pub(crate) fn trimmed_node_text(sf: Option<P<SourceFile>>, node: Option<P<Node>>) -> String {
    let (Some(node), Some(sf)) = (node, sf) else {
        return String::new();
    };
    let text = sf.text();
    let pos = i64::from(ets_shim::scanner::get_token_pos_of_node(node, sf, false));
    let end = go_end(node);
    if pos >= 0 && end >= pos && end <= text.len() as i64 {
        return ets_go::strings::trim_space(&text[pos as usize..end as usize]).to_string();
    }
    String::new()
}
