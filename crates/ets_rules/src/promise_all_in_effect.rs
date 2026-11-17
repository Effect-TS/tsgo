use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Node, SourceFile, SymbolFlags};
use ets_shim::checker::{ElementFlags, Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    EffectContextFlags, EffectMajorVersion, PipingFlow, PipingFlowTransformation, TypeParser,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::X_0_STARTS_EVERY_PROMISE_AT_ONCE_WITH_NO_CONCURRENCY_BOUND_AND_THEY_KEEP_RUNNING_WHEN_THE_FIBER_IS_INTERRUPTED_USE_1_WITH_A_CONCURRENCY_OPTION_INSTEAD_EFFECT_PROMISE_ALL_IN_EFFECT;

// go: internal/rules/promise_all_in_effect.go:16 PromiseAllInEffect 07ad15585d24f980
pub static PROMISE_ALL_IN_EFFECT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "promiseAllInEffect",
    group: "effectNative",
    description: "Warns when Promise.all or Promise.allSettled fans out over an array inside Effect code instead of Effect.forEach or Effect.all with a concurrency option",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_promise_all_in_effect(ctx.type_parser, Some(source_file));
        if matches.is_empty() {
            return Vec::new();
        }
        let mut settled_replacement = r#"Effect.all(..., { mode: "result" })"#;
        if ctx.type_parser.supported_effect_version() == EffectMajorVersion::V3 {
            settled_replacement = r#"Effect.all(..., { mode: "either" })"#;
        }
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            let mut replacement = "Effect.forEach";
            if m.method == PromiseMethod::AllSettled {
                replacement = settled_replacement;
            }
            let method = "Promise.".to_string() + m.method.as_str();
            diags.push(ctx.new_diagnostic(
                source_file,
                m.location,
                MESSAGE,
                &[],
                &[&method, replacement],
            ));
        }
        diags
    },
});

// go: internal/rules/promise_all_in_effect.go:53 PromiseMethod 4a6e30d4706267e5
// go: internal/rules/promise_all_in_effect.go:55 PromiseMethodAll 2a2187222777cadf
//
// Rust: Go's empty `PromiseMethod` (no combinator) is `None` of an `Option<PromiseMethod>`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PromiseMethod {
    All,
    AllSettled,
}

// go: internal/rules/promise_all_in_effect.go:53 PromiseMethod 4a6e30d4706267e5
impl PromiseMethod {
    /// Rust-only: the Go string value (`string(method)`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::AllSettled => "allSettled",
        }
    }
}

// go: internal/rules/promise_all_in_effect.go:60 PromiseAllInEffectMatch d0a991b2d1cc96fe
//
// Rust: Go's `*typeparser.PipingFlow` (into the parser's cached flows) is a clone of the flow.
#[derive(Clone)]
pub struct PromiseAllInEffectMatch {
    pub location: TextRange,
    pub method: PromiseMethod,
    pub flow: PipingFlow,
    pub index: i64,
}

// go: internal/rules/promise_all_in_effect.go:67 AnalyzePromiseAllInEffect 59265455d035f770
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`). Go's `memberNode`,
// `isGlobalPromiseReceiver` and `combinatorName` closures are nested functions that take what they capture.
pub fn analyze_promise_all_in_effect(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<PromiseAllInEffectMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    let Some(global_promise) =
        tp.checker().resolve_name_exported("Promise", None, SymbolFlags::Value, false)
    else {
        return Vec::new();
    };
    fn member_node(callee: Option<P<Node>>) -> Option<P<Node>> {
        if let Some(callee) = callee
            && (ast::is_property_access_expression(callee)
                || ast::is_element_access_expression(callee))
        {
            return ast::get_element_or_property_access_name(callee);
        }
        None
    }
    fn is_global_promise_receiver(
        tp: &mut TypeParser<'_>,
        global_promise: P<ast::Symbol>,
        callee: P<Node>,
    ) -> bool {
        let receiver = callee.expression().map(ast::skip_parentheses);
        let symbol = tp.get_symbol_at_location(receiver);
        tp.resolve_to_global_symbol(symbol) == Some(global_promise)
    }
    fn combinator_name(
        tp: &mut TypeParser<'_>,
        transformation: &PipingFlowTransformation,
    ) -> Option<PromiseMethod> {
        let symbol = tp.reference_symbol_at_node(member_node(Some(transformation.callee())))?;
        match symbol.name() {
            "all" => Some(PromiseMethod::All),
            "allSettled" => Some(PromiseMethod::AllSettled),
            _ => None,
        }
    }
    let is_candidate = |tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| {
        combinator_name(tp, transformation).is_some()
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        let sequences = PipingFlow::find_transformation_sequences(Some(flow), tp, &[&is_candidate]);
        for sequence in sequences.iter().flatten() {
            let index = sequence.start();
            let transformation = flow.transformations().at(index);
            let callee = transformation.callee();
            let Some(method) = combinator_name(tp, transformation) else {
                continue;
            };
            if !is_global_promise_receiver(tp, global_promise, callee)
                || !tp.is_node_reference_to_global_member(
                    member_node(Some(callee)),
                    "Promise",
                    method.as_str(),
                )
                || !tp.get_effect_context_flags(callee).intersects(EffectContextFlags::InEffect)
                || {
                    let input_type = PipingFlow::transformation_input_type(Some(flow), tp, index);
                    !is_variable_length_array_type(tp, input_type)
                }
            {
                continue;
            }
            matches.push(PromiseAllInEffectMatch {
                location: ets_shim::scanner::get_error_range_for_node(sf, callee),
                method,
                flow: flow.clone(),
                index,
            });
        }
    }
    matches
}

// go: internal/rules/promise_all_in_effect.go:123 isVariableLengthArrayType 295c8c9a55fb4bd1
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn is_variable_length_array_type(tp: &mut TypeParser<'_>, t: Option<P<Type>>) -> bool {
    let Some(t) = t else {
        return false;
    };
    if t.flags().intersects(TypeFlags::Union) {
        for member in tp.unroll_union_members(t).unwrap_or_default() {
            if !is_variable_length_array_type(tp, Some(member)) {
                return false;
            }
        }
        return true;
    }
    if t.flags().intersects(TypeFlags::Intersection) {
        return is_variable_length_intersection(tp, t);
    }
    if t.flags().intersects(TypeFlags::Instantiable) {
        let constraint = tp.checker().get_base_constraint_of_type(t);
        return constraint.is_some_and(|constraint| {
            constraint != t && is_variable_length_array_type(tp, Some(constraint))
        });
    }
    if tp.array_type(t).is_some() {
        return true;
    }
    ets_shim::checker::is_tuple_type_exported(t) && tuple_has_variable_element(t)
}

// go: internal/rules/promise_all_in_effect.go:148 isVariableLengthIntersection 0d2d80f3b9c2c584
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn is_variable_length_intersection(tp: &mut TypeParser<'_>, t: P<Type>) -> bool {
    let constituents = t.types();
    !constituents.iter().any(|&constituent| is_fixed_length_tuple(constituent))
        && constituents
            .iter()
            .any(|&constituent| is_variable_length_array_type(tp, Some(constituent)))
}

// go: internal/rules/promise_all_in_effect.go:156 isFixedLengthTuple 7043812807e39867
pub(crate) fn is_fixed_length_tuple(t: P<Type>) -> bool {
    ets_shim::checker::is_tuple_type_exported(t) && !tuple_has_variable_element(t)
}

// go: internal/rules/promise_all_in_effect.go:160 tupleHasVariableElement a1f2182de27608a9
//
// Rust: Go's `t.TargetTupleType()` (`AsTypeReference().target.AsTupleType()`) is `t.target()` (the reference's
// object-type target) viewed as a tuple type; `TargetTupleType` is crate-private in tsrs.
pub(crate) fn tuple_has_variable_element(t: P<Type>) -> bool {
    let target = t.target().expect("tuple type reference without a target");
    for flags in target.as_tuple_type().element_flags() {
        if flags.intersects(ElementFlags::Variable) {
            return true;
        }
    }
    false
}
