use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    ObjectLiteralPropertyFlags, PipingFlow, PipingFlowTransformation, TransformationKind,
    TypeParser,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_0_RUNS_SEQUENTIALLY_UNLESS_CONCURRENCY_IS_SET_PASS_CONCURRENCY_COLON_1_TO_KEEP_SEQUENTIAL_EXECUTION_OR_SET_THE_CONCURRENCY_YOU_INTEND_EFFECT_MISSING_CONCURRENCY_OPTION;

// go: internal/rules/missing_concurrency_option.go:16 MissingConcurrencyOption 76c625fe7e78241c
pub static MISSING_CONCURRENCY_OPTION: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "missingConcurrencyOption",
    group: "style",
    description: "Reports Effect.all and Effect.forEach calls without an explicit concurrency option, which run sequentially by default",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_missing_concurrency_option(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.api.as_str()],
            ));
        }
        diags
    },
});

// go: internal/rules/missing_concurrency_option.go:41 ConcurrencyApi c4466ac690f1bc85
// go: internal/rules/missing_concurrency_option.go:43 ConcurrencyApiAll 01e23175a3448aba
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConcurrencyApi {
    All,
    ForEach,
}

// go: internal/rules/missing_concurrency_option.go:41 ConcurrencyApi c4466ac690f1bc85
impl ConcurrencyApi {
    /// Rust-only: the Go string value (`string(api)`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::ForEach => "forEach",
        }
    }
}

// go: internal/rules/missing_concurrency_option.go:48 MissingConcurrencyOptionMatch e3f37409367a6eaf
//
// Rust: Go's `*typeparser.PipingFlow` (into the parser's cached flows) is a clone of the flow.
#[derive(Clone)]
pub struct MissingConcurrencyOptionMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub api: ConcurrencyApi,
    pub flow: PipingFlow,
    pub step_index: i64,
    pub options_literal: Option<P<Node>>,
    // OptionsUnderSatisfies is set when a `satisfies` wraps the options
    // literal: adding `concurrency` there may break the satisfies check.
    pub options_under_satisfies: bool,
}

// go: internal/rules/missing_concurrency_option.go:60 AnalyzeMissingConcurrencyOption 44f7dfdbb6f89036
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`); the parser and the source file are never
// nil.
pub fn analyze_missing_concurrency_option(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<MissingConcurrencyOptionMatch> {
    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for (_, flow) in ets_readonly::new(flows.as_deref()).all() {
        for (index, step) in flow.transformations().all() {
            let callee = step.callee();
            if !is_invoked_callee(Some(callee)) {
                continue;
            }
            let Some(api) = concurrency_api_of(tp, callee) else {
                continue;
            };
            let Some(options) = missing_concurrency_options_argument(api, step) else {
                continue;
            };
            let mut literal = None;
            let mut under_satisfies = false;
            if let Some(options) = options {
                let (_, flags) = ets_typeparser::object_literal_property(options, "concurrency");
                if flags != ObjectLiteralPropertyFlags::None {
                    continue;
                }
                literal =
                    Some(ast::skip_outer_expressions(options, ast::OuterExpressionKinds::All));
                under_satisfies = is_under_satisfies(options);
            }
            matches.push(MissingConcurrencyOptionMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, callee),
                api,
                flow: flow.clone(),
                step_index: index,
                options_literal: literal,
                options_under_satisfies: under_satisfies,
            });
        }
    }
    // Go: sort.SliceStable.
    matches.sort_by_key(|m| m.location.pos());
    matches
}

// go: internal/rules/missing_concurrency_option.go:106 isInvokedCallee 8b98c73a54a76a1e
pub(crate) fn is_invoked_callee(callee: Option<P<Node>>) -> bool {
    let Some(callee) = callee else {
        return false;
    };
    callee.parent().is_some_and(|parent| {
        ast::is_call_expression(parent) && parent.as_call_expression().expression() == callee
    })
}

// go: internal/rules/missing_concurrency_option.go:111 concurrencyApiOf 46fdaf8422780adb
pub(crate) fn concurrency_api_of(
    tp: &mut TypeParser<'_>,
    callee: P<Node>,
) -> Option<ConcurrencyApi> {
    [ConcurrencyApi::ForEach, ConcurrencyApi::All]
        .into_iter()
        .find(|&api| tp.is_node_reference_to_effect_module_api(callee, api.as_str()))
}

// go: internal/rules/missing_concurrency_option.go:120 IsInputOnlyCall 9750f86a4b57a2f3
pub fn is_input_only_call(step: &PipingFlowTransformation) -> bool {
    step.kind() == TransformationKind::Call && step.args().len() == 0
}

// go: internal/rules/missing_concurrency_option.go:124 forEachOptionsIndex 008351c4a7779782
pub(crate) const FOR_EACH_OPTIONS_INDEX: i64 = 1;

// go: internal/rules/missing_concurrency_option.go:126 missingConcurrencyOptionsArgument 67b27644afa59231
// Rust: Go's `(*ast.Node, bool)` is `Option<Option<P<Node>>>`: `None` when Go returns false, `Some(None)` for its
// nil node.
#[expect(
    clippy::option_option,
    reason = "Go's (*ast.Node, bool) result, where the node may be nil"
)]
pub(crate) fn missing_concurrency_options_argument(
    api: ConcurrencyApi,
    step: &PipingFlowTransformation,
) -> Option<Option<P<Node>>> {
    let args = step.args();
    if is_input_only_call(step) {
        return Some(None);
    } else if step.kind() == TransformationKind::CallWithOptionals {
        if args.len() == 1 {
            return Some(Some(*args.at(0)));
        }
    } else if api == ConcurrencyApi::ForEach && for_each_takes_callback_argument(step.kind()) {
        if args.len() <= FOR_EACH_OPTIONS_INDEX {
            return Some(None);
        } else if args.len() == FOR_EACH_OPTIONS_INDEX + 1 {
            return Some(Some(*args.at(FOR_EACH_OPTIONS_INDEX)));
        }
    }
    None
}

// go: internal/rules/missing_concurrency_option.go:146 forEachTakesCallbackArgument f47d7b292552146f
pub(crate) const fn for_each_takes_callback_argument(kind: TransformationKind) -> bool {
    matches!(
        kind,
        TransformationKind::DataFirst
            | TransformationKind::Pipe
            | TransformationKind::Pipeable
            | TransformationKind::Call
    )
}

// go: internal/rules/missing_concurrency_option.go:157 isUnderSatisfies 033775c6f81a0a6e
pub(crate) fn is_under_satisfies(mut node: P<Node>) -> bool {
    while ast::is_outer_expression(node, ast::OuterExpressionKinds::All) {
        if ast::is_satisfies_expression(node) {
            return true;
        }
        node = node.expression().unwrap();
    }
    false
}
