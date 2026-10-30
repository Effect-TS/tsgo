use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, NodeFlags, SourceFile, Symbol};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    LazyExpressionFlags, ParsedLazyExpression, PipingFlow, PipingFlowTransformation,
    TransformationKind, TypeParser, parse_lazy_expression,
};
use rustc_hash::FxHashSet;

use crate::piping_flow::has_explicit_type_arguments;
use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_NESTED_EFFECT_FLAT_MAP_SLASH_EFFECT_MAP_ONLY_PAIRS_TWO_INDEPENDENT_EFFECTS_USE_EFFECT_ALL_SEQUENTIAL_BY_DEFAULT_TO_EXPRESS_THIS_DIRECTLY_EFFECT_ZIP_VIA_NESTED_FLAT_MAP_TO_ALL;

// go: internal/rules/zip_via_nested_flat_map_to_all.go:19 ZipViaNestedFlatMapToAll ef06fbd59ea75ea8
// ZipViaNestedFlatMapToAll suggests Effect.all when an Effect.flatMap callback
// only maps an independent effect and pairs both results into a tuple or a
// two-key record.
pub static ZIP_VIA_NESTED_FLAT_MAP_TO_ALL: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "zipViaNestedFlatMapToAll",
    group: "style",
    description: "Suggests Effect.all when a nested Effect.flatMap/Effect.map only pairs two independent effects",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_zip_via_nested_flat_map_to_all(ctx.type_parser, source_file);
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diagnostics
    },
});

// go: internal/rules/zip_via_nested_flat_map_to_all.go:46 ZipViaNestedFlatMapToAllOperand bed03adb30e0038b
// ZipViaNestedFlatMapToAllOperand is the effect consumed by a flow step, which
// the fix passes to Effect.all: either a standalone expression, or the first
// Count arguments of the pipe call that applies the step.
//
// Rust: the pipe call is its call expression node (Go's `*ast.CallExpression`).
#[derive(Clone, Copy, Default)]
pub struct ZipViaNestedFlatMapToAllOperand {
    pub node: Option<P<Node>>,
    pub pipe_call: Option<P<Node>>,
    pub count: usize,
}

impl ZipViaNestedFlatMapToAllOperand {
    // go: internal/rules/zip_via_nested_flat_map_to_all.go:53 ZipViaNestedFlatMapToAllOperand.Parts 70ce1308e36acdae
    // Parts returns the source nodes evaluated to build the operand.
    pub fn parts(self) -> Vec<P<Node>> {
        if let Some(node) = self.node {
            return vec![node];
        }
        let Some(pipe_call) = self.pipe_call else {
            return Vec::new();
        };
        let call = pipe_call.as_call_expression();
        let mut parts = vec![call.expression()];
        parts.extend_from_slice(&call.arguments().nodes()[..self.count]);
        parts
    }
}

// go: internal/rules/zip_via_nested_flat_map_to_all.go:66 ZipViaNestedFlatMapToAllMatch 4744c141e660190d
// ZipViaNestedFlatMapToAllMatch holds the nodes needed by the diagnostic and
// quick fix. The fix replaces the outer flow up to and including the flatMap
// step with Effect.all; it is only offered when Fixable is set.
//
// Rust: Go's `*PipingFlow` is an owned copy.
#[derive(Clone)]
pub struct ZipViaNestedFlatMapToAllMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub callee: P<Node>,
    pub flow: PipingFlow,
    pub index: i64,
    pub a: ZipViaNestedFlatMapToAllOperand,
    pub b: ZipViaNestedFlatMapToAllOperand,
    pub record: bool,
    pub keys: Vec<P<Node>>,
    pub fixable: bool,
}

// go: internal/rules/zip_via_nested_flat_map_to_all.go:83 AnalyzeZipViaNestedFlatMapToAll 826085741d862f34
// AnalyzeZipViaNestedFlatMapToAll finds Effect.flatMap steps whose callback
// returns a flow ending in Effect.map over an effect that does not depend on
// the flatMap value, with a combiner that only packs both values into [x, y]
// or { k1: x, k2: y }.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`). The parsed callbacks hold a non-optional
// expression, and `LongestPipingFlowAt` never returns nil for an expression, so Go's nil checks drop.
pub fn analyze_zip_via_nested_flat_map_to_all(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<ZipViaNestedFlatMapToAllMatch> {
    let is_step = |name: &'static str| {
        move |tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| -> bool {
            zip_callback(step).is_some()
                && tp.is_node_reference_to_effect_module_api(step.callee(), name)
        }
    };
    let (is_flat_map, is_map) = (is_step("flatMap"), is_step("map"));

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for (_, flow) in ets_readonly::new(flows.as_deref()).all() {
        let sequences = PipingFlow::find_transformation_sequences(Some(flow), tp, &[&is_flat_map]);
        for (_, sequence) in ets_readonly::new(sequences.as_deref()).all() {
            let index = sequence.start();
            let step = flow.transformations().at(index);
            let callee = step.callee();
            let Some(outer) = zip_callback(step) else {
                continue;
            };

            // The callback returns a flow whose last step is Effect.map; the
            // effect that step consumes is the second operand.
            let Some(inner) = tp.longest_piping_flow_at(outer.expression(), false) else {
                continue;
            };
            if inner.transformations().len() == 0 {
                continue;
            }
            let last = inner.transformations().len() - 1;
            let inner_step = inner.transformations().at(last).clone();
            if !is_map(tp, &inner_step) {
                continue;
            }
            let Some(inner_callback) = zip_callback(&inner_step) else {
                continue;
            };

            let outer_symbol = tp.get_symbol_at_location(outer.params().at(0).name());
            let inner_symbol = tp.get_symbol_at_location(inner_callback.params().at(0).name());
            let (Some(outer_symbol), Some(inner_symbol)) = (outer_symbol, inner_symbol) else {
                continue;
            };
            if outer_symbol == inner_symbol {
                continue;
            }
            let Some((record, keys)) = zip_combiner_shape(
                tp,
                Some(inner_callback.expression()),
                outer_symbol,
                inner_symbol,
            ) else {
                continue;
            };

            let Some(b) = zip_step_input(tp, &inner, last) else {
                continue;
            };
            if !zip_operand_independent_of(tp, b, outer_symbol) {
                continue;
            }
            if b.parts()
                .into_iter()
                .any(|part| !tp.is_expression_evaluable_at_location(part, callee))
                || {
                    let input_type = PipingFlow::transformation_input_type(Some(flow), tp, index);
                    tp.strict_effect_type(input_type).is_none()
                }
                || {
                    let input_type = PipingFlow::transformation_input_type(Some(&inner), tp, last);
                    tp.strict_effect_type(input_type).is_none()
                }
            {
                continue;
            }

            // The input of an Effect.fn pipeline step is the generator body's
            // result, which has no expression to pass to Effect.all.
            let resolved = zip_step_input(tp, flow, index);
            let a = resolved.unwrap_or_default();
            matches.push(ZipViaNestedFlatMapToAllMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, callee),
                callee,
                flow: flow.clone(),
                index,
                a,
                b,
                record,
                keys,
                fixable: resolved.is_some()
                    && !has_explicit_type_arguments(Some(step))
                    && !has_explicit_type_arguments(Some(&inner_step))
                    && zip_operand_clonable(a)
                    && zip_operand_clonable(b),
            });
        }
    }

    matches
}

// go: internal/rules/zip_via_nested_flat_map_to_all.go:156 zipCallback 66a565ea1cac8233
// zipCallback returns the single arrow callback of a step when it declares one
// plain identifier parameter and returns an expression.
//
// Rust: the step's callee and arguments are never nil, and a parsed callback always has an expression, so Go's nil
// checks on them drop.
pub(crate) fn zip_callback(step: &PipingFlowTransformation) -> Option<ParsedLazyExpression> {
    let callee = step.callee();
    if callee.flags().intersects(NodeFlags::OptionalChain) || step.args().len() != 1 {
        return None;
    }
    let argument = *step.args().at(0);
    if argument.kind() == Kind::SpreadElement {
        return None;
    }
    let callback = parse_lazy_expression(argument, LazyExpressionFlags::None)?;
    if callback.node().kind() != Kind::ArrowFunction || callback.params().len() != 1 {
        return None;
    }
    let parameter = *callback.params().at(0);
    if !plain_tap_parameter(parameter)
        || parameter.as_parameter_declaration().question_token().is_some()
    {
        return None;
    }
    Some(callback)
}

// go: internal/rules/zip_via_nested_flat_map_to_all.go:180 zipCombinerShape a79638f542f0c047
// zipCombinerShape reports whether combiner is exactly [x, y] (optionally
// `as const`) or a two-property object literal whose values are x then y.
// For records it returns the key nodes in order.
//
// Rust: Go's `(record, keys, ok)` is `Option<(record, keys)>`, and the checker is the parser's.
pub(crate) fn zip_combiner_shape(
    tp: &mut TypeParser<'_>,
    combiner: Option<P<Node>>,
    x: P<Symbol>,
    y: P<Symbol>,
) -> Option<(bool, Vec<P<Node>>)> {
    let mut combiner = combiner.map(ast::skip_parentheses);
    if let Some(node) = combiner
        && (node.kind() == Kind::AsExpression || node.kind() == Kind::TypeAssertionExpression)
        && node.type_node().is_some_and(ast::is_const_type_reference)
    {
        combiner = node.expression().map(ast::skip_parentheses);
    }
    let combiner = combiner?;
    let expected = [x, y];

    match combiner.kind() {
        Kind::ArrayLiteralExpression => {
            let elements = combiner.elements();
            if elements.len() != 2 {
                return None;
            }
            for (i, &element) in elements.iter().enumerate() {
                if element.kind() != Kind::Identifier
                    || hoist_sensitive_reference_symbol(tp, Some(element)) != Some(expected[i])
                {
                    return None;
                }
            }
            Some((false, Vec::new()))
        }

        Kind::ObjectLiteralExpression => {
            let properties = combiner.properties();
            if properties.len() != 2 {
                return None;
            }
            let mut keys = Vec::with_capacity(2);
            let mut seen: FxHashSet<&str> = FxHashSet::default();
            for (i, &property) in properties.iter().enumerate() {
                let value = match property.kind() {
                    Kind::ShorthandPropertyAssignment => {
                        if property
                            .as_shorthand_property_assignment()
                            .object_assignment_initializer()
                            .is_some()
                        {
                            return None;
                        }
                        tp.checker().get_shorthand_assignment_value_symbol(Some(property))
                    }
                    Kind::PropertyAssignment => {
                        let initializer = property.initializer().map(ast::skip_parentheses)?;
                        if initializer.kind() != Kind::Identifier {
                            return None;
                        }
                        hoist_sensitive_reference_symbol(tp, Some(initializer))
                    }
                    _ => return None,
                };
                let key = property.name();
                if value != Some(expected[i]) {
                    return None;
                }
                let key = key?;
                if key.kind() != Kind::Identifier && key.kind() != Kind::StringLiteral {
                    return None;
                }
                let text = key.text();
                // Effect.all and Object.keys visit integer-like keys first, and
                // `__proto__` sets the prototype instead of defining a key.
                if seen.contains(text) || text == "__proto__" || is_canonical_array_index(text) {
                    return None;
                }
                seen.insert(text);
                keys.push(key);
            }
            Some((true, keys))
        }

        _ => None,
    }
}

// go: internal/rules/zip_via_nested_flat_map_to_all.go:247 isCanonicalArrayIndex 79038d8be8b00f8a
pub(crate) fn is_canonical_array_index(text: &str) -> bool {
    if text.is_empty() || text.len() > 1 && text.as_bytes()[0] == b'0' {
        return false;
    }
    for ch in text.chars() {
        if !ch.is_ascii_digit() {
            return false;
        }
    }
    true
}

// go: internal/rules/zip_via_nested_flat_map_to_all.go:262 zipStepInput f88d54413001f485
// zipStepInput returns the effect consumed by the indexed step of flow. A pipe
// or pipeable step that is not the first argument of its pipe call consumes
// the same call truncated before it.
//
// Rust: Go's `(operand, ok)` is `Option<operand>`; a parsed pipe call's subject is never nil, so Go's
// `parsed.Subject() != nil` holds.
pub(crate) fn zip_step_input(
    tp: &mut TypeParser<'_>,
    flow: &PipingFlow,
    index: i64,
) -> Option<ZipViaNestedFlatMapToAllOperand> {
    let step = flow.transformations().at(index);
    match step.kind() {
        TransformationKind::EffectFn | TransformationKind::EffectFnUntraced => {
            return None;
        }
        TransformationKind::Pipe | TransformationKind::Pipeable => {
            let (call, argument_index) = zip_pipe_call_of(step)?;
            let parsed = tp.parse_pipe_call(call)?;
            let mut first = 0;
            if step.kind() == TransformationKind::Pipe {
                first = 1;
            }
            if argument_index == first {
                return Some(ZipViaNestedFlatMapToAllOperand {
                    node: Some(parsed.subject()),
                    ..Default::default()
                });
            }
            if argument_index > first {
                return Some(ZipViaNestedFlatMapToAllOperand {
                    node: None,
                    pipe_call: Some(call),
                    count: argument_index as usize,
                });
            }
            return None;
        }
        _ => {}
    }
    let input = PipingFlow::transformation_input_node(Some(flow), index)?;
    Some(ZipViaNestedFlatMapToAllOperand { node: Some(input), ..Default::default() })
}

// go: internal/rules/zip_via_nested_flat_map_to_all.go:296 zipPipeCallOf 24b45d87c8431601
// zipPipeCallOf returns the pipe call that receives a pipe or pipeable step as
// an argument, with the argument's index.
//
// Rust: Go's `(nil, -1)` is `None`; the call is its node, and its argument list is never nil in tsrs.
pub(crate) fn zip_pipe_call_of(step: &PipingFlowTransformation) -> Option<(P<Node>, i64)> {
    let mut argument = step.callee();
    if let Some(parent) = argument.parent()
        && ast::is_call_expression(parent)
        && parent.as_call_expression().expression() == argument
    {
        argument = parent;
    }
    let parent = argument.parent()?;
    if !ast::is_call_expression(parent) {
        return None;
    }
    let index = parent
        .as_call_expression()
        .arguments()
        .nodes()
        .iter()
        .position(|&node| node == argument)?;
    Some((parent, index as i64))
}

// go: internal/rules/zip_via_nested_flat_map_to_all.go:311 zipOperandClonable 1c9506a0f0a6f137
// zipOperandClonable reports whether the fix can rebuild the operand without
// dropping explicit type arguments or optional chaining of a truncated pipe call.
pub(crate) fn zip_operand_clonable(operand: ZipViaNestedFlatMapToAllOperand) -> bool {
    let Some(call) = operand.pipe_call else {
        return true;
    };
    let expression = call.as_call_expression();
    !call.flags().intersects(NodeFlags::OptionalChain)
        && !expression.expression().flags().intersects(NodeFlags::OptionalChain)
        && expression
            .type_arguments()
            .is_none_or(|type_arguments| type_arguments.nodes().is_empty())
}

// go: internal/rules/zip_via_nested_flat_map_to_all.go:320 zipOperandIndependentOf ae66e60612dd1b47
// zipOperandIndependentOf reports whether no identifier in the operand,
// including nested functions and type positions, refers to symbol.
//
// Rust: the checker is the parser's.
pub(crate) fn zip_operand_independent_of(
    tp: &mut TypeParser<'_>,
    operand: ZipViaNestedFlatMapToAllOperand,
    symbol: P<Symbol>,
) -> bool {
    // Rust: Go's recursive `references` closure.
    fn references(tp: &mut TypeParser<'_>, node: P<Node>, symbol: P<Symbol>) -> bool {
        if node.kind() == Kind::Identifier
            && hoist_sensitive_reference_symbol(tp, Some(node)) == Some(symbol)
        {
            return true;
        }
        node.for_each_child(&mut |child| references(tp, child, symbol))
    }
    !operand.parts().into_iter().any(|part| references(tp, part, symbol))
}
