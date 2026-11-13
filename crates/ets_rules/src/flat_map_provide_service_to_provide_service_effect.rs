use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, NodeFlags, SourceFile, Symbol};
use ets_shim::checker::{Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    LazyExpressionFlags, PipingFlow, PipingFlowTransformation, TypeParser, parse_lazy_expression,
};

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_PROVIDE_SERVICE_EFFECT_PROVIDES_THIS_SERVICE_FROM_THE_ACQUIRING_EFFECT_DIRECTLY_INSTEAD_OF_EFFECT_0_FORWARDING_ITS_PARAMETER_TO_EFFECT_PROVIDE_SERVICE_EFFECT_FLAT_MAP_PROVIDE_SERVICE_TO_PROVIDE_SERVICE_EFFECT;

// go: internal/rules/flat_map_provide_service_to_provide_service_effect.go:14 FlatMapProvideServiceToProvideServiceEffect 834fca6797fd3a7a
pub static FLAT_MAP_PROVIDE_SERVICE_TO_PROVIDE_SERVICE_EFFECT: LazyLock<Rule> = LazyLock::new(
    || Rule {
        name: "flatMapProvideServiceToProvideServiceEffect",
        group: "style",
        description: "Suggests Effect.provideServiceEffect when a flatMap or andThen callback only forwards its parameter to Effect.provideService",
        default_severity: Severity::Suggestion,
        supported_effect: vec!["v3", "v4"],
        codes: vec![MESSAGE.code()],
        replaced_by: vec![],
        run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
            let source_file = ctx.source_file;
            let matches = analyze_flat_map_provide_service_to_provide_service_effect(
                ctx.type_parser,
                source_file,
            );
            let mut diagnostics = Vec::with_capacity(matches.len());
            for m in &matches {
                diagnostics.push(ctx.new_diagnostic(
                    m.source_file,
                    m.location,
                    MESSAGE,
                    &[],
                    &[m.method],
                ));
            }
            diagnostics
        },
    },
);

// go: internal/rules/flat_map_provide_service_to_provide_service_effect.go:39 FlatMapProvideServiceToProvideServiceEffectMatch 6a2a878dc39f703f
// FlatMapProvideServiceToProvideServiceEffectMatch is a flatMap or andThen step
// whose callback only provides its parameter as a service:
//
//	outer: acquire... -> [Index] flatMap((p) => inner)
//	inner: self... -> [last] provideService(Key, p)
//
// Rust: `Method` is one of two literals, so it is a `&'static str`.
#[derive(Clone)]
pub struct FlatMapProvideServiceToProvideServiceEffectMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub method: &'static str,
    pub flow: PipingFlow,
    pub index: i64,
    pub inner: PipingFlow,
    pub key: P<Node>,
}

// go: internal/rules/flat_map_provide_service_to_provide_service_effect.go:51 AnalyzeFlatMapProvideServiceToProvideServiceEffect 819bda6064238f1c
// AnalyzeFlatMapProvideServiceToProvideServiceEffect finds flatMap and andThen
// callbacks that are a manual Effect.provideServiceEffect.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`). A transformation's callee and a service's
// shape are never nil, so Go's nil checks on them drop.
pub fn analyze_flat_map_provide_service_to_provide_service_effect(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<FlatMapProvideServiceToProvideServiceEffectMatch> {
    let mut matches = Vec::new();
    let is_flat_map_or_and_then = |tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| {
        let callee = step.callee();
        !callee.flags().intersects(NodeFlags::OptionalChain)
            && step.args().len() == 1
            && (tp.is_node_reference_to_effect_module_api(callee, "flatMap")
                || tp.is_node_reference_to_effect_module_api(callee, "andThen"))
    };
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        let sequences =
            PipingFlow::find_transformation_sequences(Some(flow), tp, &[&is_flat_map_or_and_then]);
        for sequence in sequences.as_deref().unwrap_or_default() {
            let i = sequence.start();
            let step = sequence.transformations().at(0);
            let callee = step.callee();
            let mut method = "flatMap";
            if tp.is_node_reference_to_effect_module_api(callee, "andThen") {
                method = "andThen";
            }
            let callback = parse_lazy_expression(*step.args().at(0), LazyExpressionFlags::None);
            let Some(callback) = callback else {
                continue;
            };
            if callback.params().len() != 1 {
                continue;
            }
            let parameter = *callback.params().at(0);
            if !plain_tap_parameter(parameter) {
                continue;
            }
            let Some(parameter_symbol) = tp.get_symbol_at_location(parameter.name()) else {
                continue;
            };

            let returned = ast::skip_parentheses(callback.expression());
            let inner = tp.longest_piping_flow_at(returned, false);
            let Some(inner) = inner else {
                continue;
            };
            if inner.node() != returned || inner.transformations().len() == 0 {
                continue;
            }
            let last = inner.transformations().len() - 1;
            let provide = inner.transformations().at(last);
            if provide.callee().flags().intersects(NodeFlags::OptionalChain)
                || provide.args().len() != 2
                || !tp.is_node_reference_to_effect_module_api(provide.callee(), "provideService")
            {
                continue;
            }
            let implementation = ast::skip_parentheses(*provide.args().at(1));
            if implementation.kind() != Kind::Identifier
                || tp.get_symbol_at_location(implementation) != Some(parameter_symbol)
            {
                continue;
            }
            if mentions_callback_binding(
                tp,
                ast::skip_parentheses(*step.args().at(0)),
                returned,
                implementation,
            ) {
                continue;
            }

            let key = *provide.args().at(0);
            let acquired_type = PipingFlow::transformation_input_type(Some(flow), tp, i);
            let acquired = joined_strict_effect_channels(tp, acquired_type);
            let Some(acquired) = acquired else {
                continue;
            };
            let provided_input_type = PipingFlow::transformation_input_type(Some(&inner), tp, last);
            if joined_strict_effect_channels(tp, provided_input_type).is_none() {
                continue;
            }
            let mut service = tp.service_type_at_location(key);
            if service.is_none() {
                service = tp.context_tag_at_location(key);
            }
            let Some(service) = service else {
                continue;
            };
            if !tp.checker().is_type_assignable_to(acquired.a, service.shape()) {
                continue;
            }
            if !step_type_identical_to_provide_service_effect_result(tp, step, provide, &acquired) {
                continue;
            }

            matches.push(FlatMapProvideServiceToProvideServiceEffectMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, callee),
                method,
                flow: flow.clone(),
                index: i,
                inner: inner.clone(),
                key,
            });
        }
    }
    matches
}

// go: internal/rules/flat_map_provide_service_to_provide_service_effect.go:129 mentionsCallbackBinding 04999dbed9dbdce5
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`); Go's closures are nested `fn`s taking what
// they capture.
pub(crate) fn mentions_callback_binding(
    tp: &mut TypeParser<'_>,
    callback: P<Node>,
    root: P<Node>,
    skip: P<Node>,
) -> bool {
    fn resolves_to_callback_binding(
        symbol: Option<P<Symbol>>,
        node: P<Node>,
        callback: P<Node>,
        root: P<Node>,
    ) -> bool {
        declared_in_callback_outside_root(symbol, callback, root)
            || is_builtin_arguments(symbol) && callback_is_this_container(callback, node)
    }
    fn visit(
        tp: &mut TypeParser<'_>,
        node: P<Node>,
        callback: P<Node>,
        root: P<Node>,
        skip: P<Node>,
    ) -> bool {
        if node == skip {
            return false;
        }
        match node.kind() {
            Kind::ShorthandPropertyAssignment => {
                let symbol = tp.checker().get_shorthand_assignment_value_symbol(Some(node));
                if resolves_to_callback_binding(symbol, node, callback, root) {
                    return true;
                }
            }
            Kind::Identifier => {
                let symbol = tp.get_symbol_at_location(node);
                if resolves_to_callback_binding(symbol, node, callback, root) {
                    return true;
                }
            }
            Kind::ThisKeyword | Kind::SuperKeyword
                if callback_is_this_container(callback, node) =>
            {
                return true;
            }
            Kind::MetaProperty
                if node.as_meta_property().keyword_token() == Kind::NewKeyword
                    && callback_is_this_container(callback, node) =>
            {
                return true;
            }
            _ => {}
        }
        // ets:ast-walk scans the returned expression, outside the service argument, for references to the callback's parameter, own name, this, arguments or new.target
        node.for_each_child(&mut |child| visit(tp, child, callback, root, skip))
    }
    visit(tp, root, callback, root, skip)
}

// go: internal/rules/flat_map_provide_service_to_provide_service_effect.go:162 declaredInCallbackOutsideRoot 045da1a486261d15
pub(crate) fn declared_in_callback_outside_root(
    symbol: Option<P<Symbol>>,
    callback: P<Node>,
    root: P<Node>,
) -> bool {
    let Some(symbol) = symbol else {
        return false;
    };
    for &declaration in symbol.declarations() {
        if declaration.pos() >= callback.pos()
            && declaration.end() <= callback.end()
            && (declaration.pos() < root.pos() || declaration.end() > root.end())
        {
            return true;
        }
    }
    false
}

// go: internal/rules/flat_map_provide_service_to_provide_service_effect.go:175 callbackIsThisContainer 178e0c7ee1281c41
pub(crate) fn callback_is_this_container(callback: P<Node>, node: P<Node>) -> bool {
    ast::get_this_container(node, false, false) == callback
}

// go: internal/rules/flat_map_provide_service_to_provide_service_effect.go:181 isBuiltinArguments 7756df6269bb913f
// isBuiltinArguments reports whether symbol is a function's implicit arguments
// object, which the checker creates without declarations.
pub(crate) fn is_builtin_arguments(symbol: Option<P<Symbol>>) -> bool {
    symbol.is_some_and(|symbol| symbol.name() == "arguments" && symbol.declarations().is_empty())
}

// go: internal/rules/flat_map_provide_service_to_provide_service_effect.go:185 stepTypeIdenticalToProvideServiceEffectResult 988dd142b7679068
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`); the transformations are borrowed.
pub(crate) fn step_type_identical_to_provide_service_effect_result(
    tp: &mut TypeParser<'_>,
    step: &PipingFlowTransformation,
    provide: &PipingFlowTransformation,
    acquired: &EffectChannels,
) -> bool {
    if provide.type_arguments().is_some() {
        return false;
    }
    let step_type = step.out_type(tp);
    let step_channels = joined_strict_effect_channels(tp, step_type);
    let provide_type = provide.out_type(tp);
    let provided = joined_strict_effect_channels(tp, provide_type);
    let (Some(step_channels), Some(provided)) = (step_channels, provided) else {
        return false;
    };
    let c = tp.checker();
    if !c.is_type_identical_to(step_channels.a, provided.a) {
        return false;
    }
    let e = c.get_union_type_exported(&[acquired.e, provided.e]);
    if !c.is_type_identical_to(step_channels.e, e) {
        return false;
    }
    let r = c.get_union_type_exported(&[acquired.r, provided.r]);
    c.is_type_identical_to(step_channels.r, r)
}

// go: internal/rules/flat_map_provide_service_to_provide_service_effect.go:199 effectChannels c78a96e6451c5b37
#[derive(Clone, Copy)]
pub(crate) struct EffectChannels {
    pub(crate) a: P<Type>,
    pub(crate) e: P<Type>,
    pub(crate) r: P<Type>,
}

// go: internal/rules/flat_map_provide_service_to_provide_service_effect.go:203 joinedStrictEffectChannels 8a6640981daa6b8f
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn joined_strict_effect_channels(
    tp: &mut TypeParser<'_>,
    t: Option<P<Type>>,
) -> Option<EffectChannels> {
    let t = t?;
    if !t.flags().intersects(TypeFlags::Union) {
        let effect = tp.strict_effect_type(t)?;
        return Some(EffectChannels { a: effect.a(), e: effect.e(), r: effect.r() });
    }
    let members = t.types();
    let mut a = Vec::with_capacity(members.len());
    let mut e = Vec::with_capacity(members.len());
    let mut r = Vec::with_capacity(members.len());
    for &member in members {
        let effect = tp.strict_effect_type(member)?;
        a.push(effect.a());
        e.push(effect.e());
        r.push(effect.r());
    }
    let c = tp.checker();
    let a = c.get_union_type_exported(&a);
    let e = c.get_union_type_exported(&e);
    let r = c.get_union_type_exported(&r);
    Some(EffectChannels { a, e, r })
}
