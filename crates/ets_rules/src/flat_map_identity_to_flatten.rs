use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, NodeFlags, SourceFile};
use ets_shim::checker::TypeFlags;
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    LazyExpressionFlags, PipingFlow, PipingFlowTransformation, TypeParser, parse_lazy_expression,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_FLATTEN_EXPRESSES_THIS_UNWRAPPING_MORE_DIRECTLY_THAN_EFFECT_FLAT_MAP_WITH_AN_IDENTITY_CALLBACK_EFFECT_FLAT_MAP_IDENTITY_TO_FLATTEN;

// go: internal/rules/flat_map_identity_to_flatten.go:14 FlatMapIdentityToFlatten 6ae165f3ea1c793a
pub static FLAT_MAP_IDENTITY_TO_FLATTEN: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "flatMapIdentityToFlatten",
    group: "style",
    description: "Suggests Effect.flatten instead of Effect.flatMap when the callback is Effect's identity export referenced directly or an unannotated function returning its parameter",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_flat_map_identity_to_flatten(ctx.type_parser, source_file);
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diagnostics
    },
});

// go: internal/rules/flat_map_identity_to_flatten.go:34 FlatMapIdentityToFlattenMatch ed6efa4c4f195fd6
#[derive(Clone)]
pub struct FlatMapIdentityToFlattenMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub callee: P<Node>,
    pub transformation: PipingFlowTransformation,
    // EffectModuleNode is the receiver of Callee when it is an Effect module
    // property access, nil when flatMap is referenced without a module.
    pub effect_module_node: Option<P<Node>>,
}

// go: internal/rules/flat_map_identity_to_flatten.go:47 AnalyzeFlatMapIdentityToFlatten 25bc4b45342038ee
// AnalyzeFlatMapIdentityToFlatten finds Effect.flatMap steps whose callback is
// Effect's identity export referenced directly, or an unannotated arrow
// function or function expression returning its own parameter.
//
// Rust: Go's checker argument is unused and dropped. A step's callee and an Effect's success type are never nil
// here, so Go's nil checks on them drop.
pub fn analyze_flat_map_identity_to_flatten(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<FlatMapIdentityToFlattenMatch> {
    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for (_, flow) in ets_readonly::new(flows.as_deref()).all() {
        for (i, step) in flow.transformations().all() {
            let callee = step.callee();
            let args = step.args();
            if args.len() != 1
                || step.type_arguments().is_some()
                || callee.flags().intersects(NodeFlags::OptionalChain)
                || !tp.is_node_reference_to_effect_module_api(callee, "flatMap")
                || !is_identity_callback(tp, *args.at(0))
            {
                continue;
            }
            let input_type = PipingFlow::transformation_input_type(Some(flow), tp, i);
            let outer = tp.effect_type(input_type);
            let Some(outer) = outer else {
                continue;
            };
            let success = outer.a();
            if success.flags().intersects(TypeFlags::Any | TypeFlags::Never)
                || tp.effect_type(success).is_none()
            {
                continue;
            }
            let mut effect_module_node = None;
            if callee.kind() == Kind::PropertyAccessExpression {
                let receiver = callee.as_property_access_expression().expression();
                if tp.is_expression_effect_module(receiver) {
                    effect_module_node = Some(receiver);
                }
            }
            let transformation = step.clone();
            matches.push(FlatMapIdentityToFlattenMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, callee),
                callee,
                transformation,
                effect_module_node,
            });
        }
    }
    matches
}

// go: internal/rules/flat_map_identity_to_flatten.go:93 isIdentityCallback 486079f3d322bf8a
// isIdentityCallback reports whether callback names Effect's identity export
// without a local variable in between, or is an arrow function or function
// expression without type annotations whose body returns its single parameter. Annotations and typed
// aliases can change the flatMap result type, so they never match.
pub(crate) fn is_identity_callback(tp: &mut TypeParser<'_>, callback: P<Node>) -> bool {
    let callback = ast::skip_parentheses(callback);
    if tp.is_node_direct_reference_to_effect_function_module_api(callback, "identity") {
        return true;
    }
    let lazy = parse_lazy_expression(callback, LazyExpressionFlags::None);
    let Some(lazy) = lazy else {
        return false;
    };
    if lazy.params().len() != 1 || lazy.node().type_node().is_some() {
        return false;
    }
    let parameter = *lazy.params().at(0);
    let declaration = parameter.as_parameter_declaration();
    if parameter.name().is_none_or(|name| name.kind() != Kind::Identifier)
        || parameter.type_node().is_some()
        || declaration.dot_dot_dot_token().is_some()
        || parameter.initializer().is_some()
        || declaration.question_token().is_some()
    {
        return false;
    }
    let returned = ast::skip_parentheses(lazy.expression());
    if returned.kind() != Kind::Identifier {
        return false;
    }
    let symbol = tp.get_symbol_at_location(parameter.name());
    symbol.is_some_and(|symbol| tp.get_symbol_at_location(returned) == Some(symbol))
}
