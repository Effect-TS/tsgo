use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_readonly::Slice;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, CallExpression, Diagnostic, Kind, Node, SourceFile, Symbol};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    EffectMajorVersion, LazyExpressionFlags, PipingFlowTransformation, TypeParser,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::EFFECT_0_EXPRESSES_RECORDING_A_METRIC_ON_EVERY_1_DIRECTLY_EFFECT_MANUAL_METRIC_TAP_TO_TRACK;

// go: internal/rules/manual_metric_tap_to_track.go:17 ManualMetricTapToTrack 96cca4c5ba84c290
// ManualMetricTapToTrack suggests the Effect.track* combinators when a tap
// variant exists only to feed one metric through Metric.update.
pub static MANUAL_METRIC_TAP_TO_TRACK: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "manualMetricTapToTrack",
    group: "style",
    description: "Suggests Effect.trackSuccesses/trackErrors/trackDefects when an Effect.tap/tapError/tapDefect only updates a metric",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let matches = analyze_manual_metric_tap_to_track(ctx.type_parser, ctx.source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.track_api, m.channel],
            ));
        }
        diags
    },
});

// go: internal/rules/manual_metric_tap_to_track.go:44 ManualMetricTapToTrackKind 67c7f489b48f77fe
// go: internal/rules/manual_metric_tap_to_track.go:46 ManualMetricTapToTrackVerbatim 20f98f3f7d29d4b1
// ManualMetricTapToTrackKind selects the shape of the track* replacement.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ManualMetricTapToTrackKind {
    // ManualMetricTapToTrackVerbatim feeds the callback parameter as is: track*(metric).
    Verbatim,
    // ManualMetricTapToTrackMapper computes the input per run: track*(metric, (params) => input).
    Mapper,
    // ManualMetricTapToTrackConstantInput comes from tap's direct-effect overload,
    // whose input is evaluated once: trackSuccesses(Metric.withConstantInput(metric, input)).
    ConstantInput,
}

// go: internal/rules/manual_metric_tap_to_track.go:57 ManualMetricTapToTrackMatch f9a8924a2f790e82
// ManualMetricTapToTrackMatch holds the nodes needed by the diagnostic and quick fix.
//
// Rust: the transformation is never nil (Go only takes its address), so it is held by value; the parameters are
// copied out of the parsed lazy expression (Go's readonly.Slice over the same nodes).
#[derive(Clone)]
pub struct ManualMetricTapToTrackMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub transformation: PipingFlowTransformation,
    pub track_api: &'static str, // trackSuccesses, trackErrors or trackDefects
    pub channel: &'static str,   // success, error or defect
    pub kind: ManualMetricTapToTrackKind,
    pub metric: P<Node>, // The updated metric, with parentheses skipped
    pub input: P<Node>,  // The metric input as written
    pub params: Vec<P<Node>>,
}

// go: internal/rules/manual_metric_tap_to_track.go:69 manualMetricTapAPI 36893ad12920b9a3
pub(crate) struct ManualMetricTapApi {
    tap: &'static str,
    track: &'static str,
    channel: &'static str,
}

// go: internal/rules/manual_metric_tap_to_track.go:75 manualMetricTapAPIs ee01a8ac8c80914a
static MANUAL_METRIC_TAP_APIS: [ManualMetricTapApi; 3] = [
    ManualMetricTapApi { tap: "tap", track: "trackSuccesses", channel: "success" },
    ManualMetricTapApi { tap: "tapError", track: "trackErrors", channel: "error" },
    ManualMetricTapApi { tap: "tapDefect", track: "trackDefects", channel: "defect" },
];

// go: internal/rules/manual_metric_tap_to_track.go:84 AnalyzeManualMetricTapToTrack 2681413a6b698952
// AnalyzeManualMetricTapToTrack finds Effect.tap/tapError/tapDefect calls whose
// argument only performs a data-first Metric.update, either as the whole body
// of a callback or as tap's direct-effect argument.
//
// Rust: Go's unused checker argument is dropped (the parser holds the checker); Go's
// `callee == nil` and `lazy.Expression() == nil` checks drop, both are non-optional here.
pub fn analyze_manual_metric_tap_to_track(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<ManualMetricTapToTrackMatch> {
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }
    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        for (index, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(index);
            let callee = transformation.callee();
            if transformation.args().len() != 1
                || transformation
                    .type_arguments()
                    .is_some_and(|type_arguments| !type_arguments.nodes().is_empty())
            {
                continue;
            }

            let arg = *transformation.args().at(0);
            let kind;
            let update;
            let mut params = Vec::new();
            if let Some(lazy) =
                ets_typeparser::parse_lazy_expression(arg, LazyExpressionFlags::None)
            {
                if has_this_parameter(lazy.params()) {
                    continue;
                }
                // tap callbacks evaluate the metric on every run while track*
                // evaluates it once, so it must be the same value at the call site.
                let Some(call) = metric_update_call(tp, Some(lazy.expression())) else {
                    continue;
                };
                if !tp.is_expression_value_stable_at_location(call.arguments().nodes()[0], callee) {
                    continue;
                }
                update = call;
                params = lazy.params().append_to(Vec::new());
                let mut k = ManualMetricTapToTrackKind::Mapper;
                if is_parameter_verbatim(tp, lazy.params(), update.arguments().nodes()[1]) {
                    k = ManualMetricTapToTrackKind::Verbatim;
                } else if lazy.node().kind() == Kind::FunctionExpression
                    && uses_function_expression_bindings(
                        tp,
                        lazy.node(),
                        lazy.params(),
                        update.arguments().nodes()[1],
                    )
                {
                    // The mapper is an arrow function that keeps the parameters and the input.
                    continue;
                }
                kind = k;
            } else {
                // The direct-effect overload builds the update once, as track* does.
                let Some(call) = metric_update_call(tp, Some(arg)) else {
                    continue;
                };
                update = call;
                kind = ManualMetricTapToTrackKind::ConstantInput;
            }

            let api = resolve_manual_metric_tap_api(tp, callee);
            let Some(api) = api else {
                continue;
            };
            if kind == ManualMetricTapToTrackKind::ConstantInput && api.tap != "tap" {
                continue;
            }

            matches.push(ManualMetricTapToTrackMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, callee),
                transformation: transformation.clone(),
                track_api: api.track,
                channel: api.channel,
                kind,
                metric: ast::skip_parentheses(update.arguments().nodes()[0]),
                input: update.arguments().nodes()[1],
                params,
            });
        }
    }
    matches
}

// go: internal/rules/manual_metric_tap_to_track.go:152 metricUpdateCall ab3ed2964bc9c3ec
// metricUpdateCall returns node as a plain data-first Metric.update(metric, input) call.
//
// Rust: Go's nil checks on the call's expression and argument list drop, both are non-optional here.
pub(crate) fn metric_update_call(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
) -> Option<&'static CallExpression> {
    let node = ast::skip_parentheses(node?);
    if node.kind() != Kind::CallExpression {
        return None;
    }
    let call = node.as_call_expression();
    if call.question_dot_token().is_some()
        || call.type_arguments().is_some_and(|type_arguments| !type_arguments.nodes().is_empty())
        || call.arguments().nodes().len() != 2
    {
        return None;
    }
    for argument in call.arguments().nodes() {
        if argument.kind() == Kind::SpreadElement {
            return None;
        }
    }
    if !tp.is_node_reference_to_effect_metric_module_api(call.expression(), "update") {
        return None;
    }
    Some(call)
}

// go: internal/rules/manual_metric_tap_to_track.go:174 hasThisParameter 01e2d50eca02d649
pub(crate) fn has_this_parameter(params: Slice<'_, P<Node>>) -> bool {
    for (_, param) in params.all() {
        if ast::is_this_parameter(*param) {
            return true;
        }
    }
    false
}

// go: internal/rules/manual_metric_tap_to_track.go:184 isParameterVerbatim a0b556ea8c2ee4a2
// isParameterVerbatim reports whether input is the single plain callback parameter itself.
pub(crate) fn is_parameter_verbatim(
    tp: &mut TypeParser<'_>,
    params: Slice<'_, P<Node>>,
    input: P<Node>,
) -> bool {
    if params.len() != 1 {
        return false;
    }
    let param = *params.at(0);
    let declaration = param.as_parameter_declaration();
    if !param.name().is_some_and(ast::is_identifier)
        || declaration.dot_dot_dot_token().is_some()
        || declaration.initializer().is_some()
    {
        return false;
    }
    let input = ast::skip_parentheses(input);
    if !ast::is_identifier(input) {
        return false;
    }
    let symbol = tp.get_symbol_at_location(input);
    symbol.is_some() && symbol == tp.get_symbol_at_location(param.name())
}

// go: internal/rules/manual_metric_tap_to_track.go:205 usesFunctionExpressionBindings 6e3954c058d2bbaa
// usesFunctionExpressionBindings reports whether the parameters or the input of
// the function expression fn read a binding that fn itself provides: this,
// super, arguments, new.target or fn's own name. An arrow function in the
// same position resolves all of them differently.
pub(crate) fn uses_function_expression_bindings(
    tp: &mut TypeParser<'_>,
    fn_: P<Node>,
    params: Slice<'_, P<Node>>,
    input: P<Node>,
) -> bool {
    let mut self_ = None;
    if let Some(name) = fn_.name() {
        self_ = tp.get_symbol_at_location(name);
    }
    // own is false inside nested non-arrow functions, which bind their own
    // this, super, arguments and new.target; fn's name is visible everywhere.
    fn visit(
        tp: &mut TypeParser<'_>,
        self_: Option<P<Symbol>>,
        node: Option<P<Node>>,
        own: bool,
    ) -> bool {
        let Some(node) = node else {
            return false;
        };
        match node.kind() {
            Kind::ThisKeyword | Kind::SuperKeyword | Kind::MetaProperty => {
                if own {
                    return true;
                }
            }
            Kind::Identifier => {
                return own && node.text() == "arguments"
                    || self_.is_some() && tp.get_symbol_at_location(node) == self_;
            }
            Kind::ClassExpression => {
                // Computed names, heritage clauses and static blocks mix both scopes.
                if own {
                    return true;
                }
            }
            Kind::FunctionExpression
            | Kind::FunctionDeclaration
            | Kind::MethodDeclaration
            | Kind::GetAccessor
            | Kind::SetAccessor
            | Kind::Constructor => {
                let name = node.name();
                return node.for_each_child(&mut |child| {
                    // A computed method name is evaluated in the enclosing scope.
                    visit(tp, self_, Some(child), own && Some(child) == name)
                });
            }
            _ => {}
        }
        node.for_each_child(&mut |child| visit(tp, self_, Some(child), own))
    }
    for (_, param) in params.all() {
        if visit(tp, self_, Some(*param), true) {
            return true;
        }
    }
    visit(tp, self_, Some(input), true)
}

// go: internal/rules/manual_metric_tap_to_track.go:247 resolveManualMetricTapAPI 0af0435b267899a2
pub(crate) fn resolve_manual_metric_tap_api(
    tp: &mut TypeParser<'_>,
    callee: P<Node>,
) -> Option<&'static ManualMetricTapApi> {
    MANUAL_METRIC_TAP_APIS
        .iter()
        .find(|api| tp.is_node_reference_to_effect_module_api(callee, api.tap))
}
