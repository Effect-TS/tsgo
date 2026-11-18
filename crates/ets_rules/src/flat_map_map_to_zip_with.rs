use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, ModifierFlags, Node, NodeFlags, SourceFile, Symbol};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    LazyExpressionFlags, PipingFlow, PipingFlowTransformation, TypeParser, parse_lazy_expression,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_ZIP_WITH_COMBINES_THESE_TWO_INDEPENDENT_EFFECTS_DIRECTLY_INSTEAD_OF_EFFECT_FLAT_MAP_NESTING_AN_EFFECT_MAP_OVER_AN_EFFECT_THAT_DOES_NOT_USE_THE_OUTER_RESULT_EFFECT_FLAT_MAP_MAP_TO_ZIP_WITH;

// go: internal/rules/flat_map_map_to_zip_with.go:14 FlatMapMapToZipWith e1ecf0c3300c9778
pub static FLAT_MAP_MAP_TO_ZIP_WITH: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "flatMapMapToZipWith",
    group: "style",
    description: "Suggests Effect.zipWith when Effect.flatMap only maps an independent effect with its parameter",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_flat_map_map_to_zip_with(ctx.type_parser, source_file);
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diagnostics
    },
});

// go: internal/rules/flat_map_map_to_zip_with.go:34 FlatMapMapToZipWithMatch 9e9dac774127a643
#[derive(Clone)]
pub struct FlatMapMapToZipWithMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub step: PipingFlowTransformation,
    pub flat_map_callback: P<Node>,
    pub returned_flow: PipingFlow,
    pub map_callback: P<Node>,
    // MapResult is what MapCallback returns when its body is an expression or a
    // single return, and nil otherwise.
    pub map_result: Option<P<Node>>,
}

// go: internal/rules/flat_map_map_to_zip_with.go:46 AnalyzeFlatMapMapToZipWith 06db1013556992c9
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`). Go's `isEffectApiStep` and `isFlatMap`
// closures capture the parser, so they are nested functions that take it; a transformation's callee is never nil,
// so that check drops.
pub fn analyze_flat_map_map_to_zip_with(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<FlatMapMapToZipWithMatch> {
    let mut matches = Vec::new();
    fn is_effect_api_step(
        tp: &mut TypeParser<'_>,
        step: &PipingFlowTransformation,
        name: &str,
    ) -> bool {
        let callee = step.callee();
        !callee.flags().intersects(NodeFlags::OptionalChain)
            && step.args().len() == 1
            && tp.is_node_reference_to_effect_module_api(callee, name)
    }
    fn is_flat_map(tp: &mut TypeParser<'_>, step: &PipingFlowTransformation) -> bool {
        is_effect_api_step(tp, step, "flatMap")
    }

    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        let sequences = PipingFlow::find_transformation_sequences(Some(flow), tp, &[&is_flat_map]);
        for (_, sequence) in ets_readonly::new(sequences.as_deref()).all() {
            let i = sequence.start();
            let step = sequence.transformations().at(0).clone();

            let parsed_flat_map_callback =
                parse_lazy_expression(*step.args().at(0), LazyExpressionFlags::AllowTypeParameters);
            let Some(parsed_flat_map_callback) = parsed_flat_map_callback else {
                continue;
            };
            if parsed_flat_map_callback.params().len() != 1
                || !flat_map_map_to_zip_with_plain_parameter(
                    *parsed_flat_map_callback.params().at(0),
                )
            {
                continue;
            }
            let flat_map_callback = parsed_flat_map_callback.node();
            let returned = ast::skip_parentheses(parsed_flat_map_callback.expression());
            let returned_flow = tp.longest_piping_flow_at(returned, false);
            let Some(returned_flow) = returned_flow else {
                continue;
            };
            if returned_flow.node() != returned || returned_flow.transformations().len() == 0 {
                continue;
            }
            let last = returned_flow.transformations().len() - 1;
            let map_step = returned_flow.transformations().at(last).clone();
            if !is_effect_api_step(tp, &map_step, "map") {
                continue;
            }

            let map_callback = flat_map_map_to_zip_with_map_callback(*map_step.args().at(0));
            let Some(map_callback) = map_callback else {
                continue;
            };
            if !flat_map_map_to_zip_with_unary_mapper(map_callback) {
                continue;
            }
            let mut map_result = None;
            if let Some(parsed_map_callback) =
                parse_lazy_expression(map_callback, LazyExpressionFlags::AllowTypeParameters)
            {
                map_result = Some(ast::skip_parentheses(parsed_map_callback.expression()));
            }

            let mut outer_symbols: Vec<P<Symbol>> = Vec::new();
            let entries =
                flat_map_callback.locals().map(|locals| locals.entries()).unwrap_or_default();
            for (_, symbol) in entries {
                outer_symbols.push(symbol);
            }
            if outer_symbols.is_empty() {
                continue;
            }
            if flat_map_map_to_zip_with_that_depends_on_outer(
                tp,
                flat_map_callback,
                &returned_flow,
                &outer_symbols,
            ) {
                continue;
            }
            if !flat_map_map_to_zip_with_references_any(tp, map_callback, &outer_symbols) {
                continue;
            }
            if flat_map_map_to_zip_with_reyields_outer(tp, flat_map_callback, map_result) {
                continue;
            }

            let input_type = PipingFlow::transformation_input_type(Some(flow), tp, i);
            if tp.strict_effect_type(input_type).is_none() {
                continue;
            }
            let returned_input_type =
                PipingFlow::transformation_input_type(Some(&returned_flow), tp, last);
            if tp.strict_effect_type(returned_input_type).is_none() {
                continue;
            }

            matches.push(FlatMapMapToZipWithMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, step.callee()),
                step,
                flat_map_callback,
                returned_flow,
                map_callback,
                map_result,
            });
        }
    }
    matches
}

// go: internal/rules/flat_map_map_to_zip_with.go:122 flatMapMapToZipWithPlainParameter 25e62a13050d6c8b
//
// Rust: the parameter is a node of a parameter list, never nil, so Go's nil check drops.
pub(crate) fn flat_map_map_to_zip_with_plain_parameter(parameter: P<Node>) -> bool {
    if parameter.kind() != Kind::Parameter {
        return false;
    }
    let declaration = parameter.as_parameter_declaration();
    if declaration.dot_dot_dot_token().is_some() || parameter.initializer().is_some() {
        return false;
    }
    let Some(name) = parameter.name() else {
        return false;
    };
    name.kind() == Kind::Identifier
        || name.kind() == Kind::ObjectBindingPattern
        || name.kind() == Kind::ArrayBindingPattern
}

// go: internal/rules/flat_map_map_to_zip_with.go:137 flatMapMapToZipWithThatParts 465519f034a09224
//
// Rust: the subject's node is an `Option` (Go's slice may hold nil; the caller skips it); a transformation's
// callee is never nil.
pub(crate) fn flat_map_map_to_zip_with_that_parts(
    returned_flow: &PipingFlow,
) -> Vec<Option<P<Node>>> {
    let mut parts = vec![returned_flow.subject().node()];
    let steps = returned_flow.transformations();
    for j in 0..steps.len() - 1 {
        let step = steps.at(j);
        parts.push(Some(step.callee()));
        for (_, &argument) in step.args().all() {
            parts.push(Some(argument));
        }
    }
    parts
}

// go: internal/rules/flat_map_map_to_zip_with.go:150 flatMapMapToZipWithThatDependsOnOuter 414a8ceb975fe962
pub(crate) fn flat_map_map_to_zip_with_that_depends_on_outer(
    tp: &mut TypeParser<'_>,
    flat_map_callback: P<Node>,
    returned_flow: &PipingFlow,
    outer_symbols: &[P<Symbol>],
) -> bool {
    for part in flat_map_map_to_zip_with_that_parts(returned_flow) {
        let Some(part) = part else {
            continue;
        };
        if flat_map_map_to_zip_with_references_any(tp, part, outer_symbols)
            || tp.uses_function_own_bindings(flat_map_callback, part)
        {
            return true;
        }
    }
    false
}

// go: internal/rules/flat_map_map_to_zip_with.go:162 flatMapMapToZipWithReferencesAny e22d125c55dec6b3
pub(crate) fn flat_map_map_to_zip_with_references_any(
    tp: &mut TypeParser<'_>,
    root: P<Node>,
    symbols: &[P<Symbol>],
) -> bool {
    for &symbol in symbols {
        if !tp.references_to_symbol(root, symbol).is_empty() {
            return true;
        }
    }
    false
}

// go: internal/rules/flat_map_map_to_zip_with.go:171 flatMapMapToZipWithReyieldsOuter df461a6efd9ff5ea
//
// Rust: the outer parameter's name is set (checked by `flat_map_map_to_zip_with_plain_parameter`), as Go assumes.
pub(crate) fn flat_map_map_to_zip_with_reyields_outer(
    tp: &mut TypeParser<'_>,
    flat_map_callback: P<Node>,
    map_result: Option<P<Node>>,
) -> bool {
    let name =
        flat_map_callback.parameters()[0].name().expect("the outer parameter is a plain parameter");
    if name.kind() != Kind::Identifier {
        return false;
    }
    let Some(map_result) = map_result else {
        return false;
    };
    map_result.kind() == Kind::Identifier
        && tp.get_symbol_at_location(map_result) == tp.get_symbol_at_location(name)
}

// go: internal/rules/flat_map_map_to_zip_with.go:179 flatMapMapToZipWithUnaryMapper 626c6e5add2b9bfa
pub(crate) fn flat_map_map_to_zip_with_unary_mapper(mapper: P<Node>) -> bool {
    let mut values = 0;
    for &parameter in mapper.parameters() {
        if ast::is_this_parameter(parameter) {
            continue;
        }
        values += 1;
        if values > 1 || !flat_map_map_to_zip_with_plain_parameter(parameter) {
            return false;
        }
    }
    true
}

// go: internal/rules/flat_map_map_to_zip_with.go:196 flatMapMapToZipWithMapCallback 998daecccf0f398a
// flatMapMapToZipWithMapCallback accepts any body shape, unlike
// ParseLazyExpression: the whole body moves into the combiner, so a mapper that
// branches between several returns is still fine.
//
// Rust: a node is never nil, so Go's nil check drops; Go's `ast.IsArrowFunction` /
// `ast.IsFunctionExpression` are kind comparisons.
pub(crate) fn flat_map_map_to_zip_with_map_callback(node: P<Node>) -> Option<P<Node>> {
    let node = ast::skip_parentheses(node);
    let is_non_generator_function = node.kind() == Kind::ArrowFunction
        || node.kind() == Kind::FunctionExpression
            && node.as_function_expression().asterisk_token().is_none();
    if !is_non_generator_function
        || node.body().is_none()
        || ast::get_combined_modifier_flags(node).intersects(ModifierFlags::Async)
    {
        return None;
    }
    Some(node)
}
