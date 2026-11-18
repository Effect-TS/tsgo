use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::PipingFlowTransformationReplacement;
use ets_rules::FlatMapMapToZipWithMatch;
use ets_shim::P;
use ets_shim::ast::{self, Kind, Node, NodeFactory, SymbolFlags, SymbolTable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::TypeParser;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_ZIP_WITH_COMBINES_THESE_TWO_INDEPENDENT_EFFECTS_DIRECTLY_INSTEAD_OF_EFFECT_FLAT_MAP_NESTING_AN_EFFECT_MAP_OVER_AN_EFFECT_THAT_DOES_NOT_USE_THE_OUTER_RESULT_EFFECT_FLAT_MAP_MAP_TO_ZIP_WITH;

// go: internal/fixables/flat_map_map_to_zip_with.go:15 FlatMapMapToZipWithFix 8e7750ba8111d302
pub static FLAT_MAP_MAP_TO_ZIP_WITH_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "flatMapMapToZipWith",
    description: "Replace Effect.flatMap with Effect.zipWith",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["flatMapMapToZipWith_fix"],
    run: run_flat_map_map_to_zip_with_fix,
});

// go: internal/fixables/flat_map_map_to_zip_with.go:25 runFlatMapMapToZipWithFix 07c4d3139160ce38
//
// Rust: the fix action's closure moves the match and the `moduleMember` (its predicates are boxed and not
// `Clone`); `resolveModuleMember` only borrows it before.
fn run_flat_map_map_to_zip_with_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let matches =
        ets_rules::analyze_flat_map_map_to_zip_with(&mut ctx.type_parser, ctx.source_file);
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if !flat_map_map_to_zip_with_fix_is_safe(ctx, &m) {
            continue;
        }
        let mut name = "zipWith";
        if flat_map_map_to_zip_with_map_callback_is_zip(&mut ctx.type_parser, &m) {
            name = "zip";
        }
        let member = effect_module_member(name);
        let target = resolve_module_member(
            ctx,
            m.step.callee(),
            &ets_typeparser::find_effect_module_reference(Some(m.source_file)),
            &member,
        );
        let Some(target) = target else {
            continue;
        };
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: format!("Replace with Effect.{name}"),
            run: Box::new(move |tracker| {
                let mut arguments = vec![
                    tracker
                        .deep_clone_node(m.returned_flow.subject().node())
                        .expect("a piping flow has a subject"),
                ];
                if name == "zipWith" {
                    arguments.push(flat_map_map_to_zip_with_combiner(tracker, &m));
                }
                let callee = target.expression(tracker, &member);
                let arguments = tracker.new_node_list(arguments);
                tracker.replace_piping_flow_transformation(
                    Some(m.source_file),
                    Some(&m.step),
                    PipingFlowTransformationReplacement {
                        callee: Some(callee),
                        type_arguments: None,
                        arguments: Some(arguments),
                    },
                );
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}

// go: internal/fixables/flat_map_map_to_zip_with.go:61 flatMapMapToZipWithFixIsSafe a3657184dfe7b10f
fn flat_map_map_to_zip_with_fix_is_safe(
    ctx: &mut Context<'_, '_>,
    m: &FlatMapMapToZipWithMatch,
) -> bool {
    flat_map_map_to_zip_with_that_can_run_first(&mut ctx.type_parser, m)
        && !flat_map_map_to_zip_with_drops_explicit_types(m)
        && !flat_map_map_to_zip_with_callbacks_declare_type_parameters(m)
        && !flat_map_map_to_zip_with_outer_parameter_is_destructured(m)
        && flat_map_map_to_zip_with_parameter_lists_can_concatenate(m)
        && !flat_map_map_to_zip_with_map_parameters_capture_outer_names(ctx, m)
        && !flat_map_map_to_zip_with_map_callback_is_tied_to_its_function(ctx, m)
}

// go: internal/fixables/flat_map_map_to_zip_with.go:71 flatMapMapToZipWithThatCanRunFirst 61bfd76ea29a530d
fn flat_map_map_to_zip_with_that_can_run_first(
    tp: &mut TypeParser<'_>,
    m: &FlatMapMapToZipWithMatch,
) -> bool {
    if m.returned_flow.transformations().len() != 1 {
        return false;
    }
    let that = m.returned_flow.subject().node();
    tp.is_expression_value_stable_at_location(that, m.step.callee())
        || tp.is_declaration_file_module_constant(that)
}

// go: internal/fixables/flat_map_map_to_zip_with.go:79 flatMapMapToZipWithDropsExplicitTypes cfaeed473bd7067f
fn flat_map_map_to_zip_with_drops_explicit_types(m: &FlatMapMapToZipWithMatch) -> bool {
    m.step.type_arguments().is_some()
        || m.returned_flow.transformations().at(0).type_arguments().is_some()
        || m.flat_map_callback.type_node().is_some()
}

// go: internal/fixables/flat_map_map_to_zip_with.go:85 flatMapMapToZipWithCallbacksDeclareTypeParameters eddeb5ab5ab86c47
fn flat_map_map_to_zip_with_callbacks_declare_type_parameters(
    m: &FlatMapMapToZipWithMatch,
) -> bool {
    !m.flat_map_callback.type_parameters().is_empty()
        || !m.map_callback.type_parameters().is_empty()
}

// go: internal/fixables/flat_map_map_to_zip_with.go:89 flatMapMapToZipWithOuterParameterIsDestructured b41ee41df8755550
//
// Rust: the outer parameter's name is set (the rule accepts only plain parameters), as Go assumes.
fn flat_map_map_to_zip_with_outer_parameter_is_destructured(m: &FlatMapMapToZipWithMatch) -> bool {
    m.flat_map_callback.parameters()[0]
        .name()
        .expect("the outer parameter is a plain parameter")
        .kind()
        != Kind::Identifier
}

// go: internal/fixables/flat_map_map_to_zip_with.go:93 flatMapMapToZipWithParameterListsCanConcatenate 51a99839ccd19e74
fn flat_map_map_to_zip_with_parameter_lists_can_concatenate(m: &FlatMapMapToZipWithMatch) -> bool {
    let outer_parameter = m.flat_map_callback.parameters()[0];
    let map_parameters = m.map_callback.parameters();
    // A `this` parameter is not a value parameter and must stay first, so the
    // two parameter lists cannot be concatenated around it (TS2680).
    if std::iter::once(&outer_parameter)
        .chain(map_parameters)
        .any(|&parameter| ast::is_this_parameter(parameter))
    {
        return false;
    }
    // The outer parameter becomes the first of the combiner's parameters, and a
    // required parameter cannot follow an optional one (TS1016).
    if outer_parameter.as_parameter_declaration().question_token().is_some() {
        for &parameter in map_parameters {
            if parameter.as_parameter_declaration().question_token().is_none() {
                return false;
            }
        }
    }
    true
}

// go: internal/fixables/flat_map_map_to_zip_with.go:113 flatMapMapToZipWithMapParametersCaptureOuterNames 9513a020b4dd42c9
//
// Rust: Go ranges over the map callback's locals (a map); the result does not depend on the order, since any hit
// returns true.
fn flat_map_map_to_zip_with_map_parameters_capture_outer_names(
    ctx: &mut Context<'_, '_>,
    m: &FlatMapMapToZipWithMatch,
) -> bool {
    let flat_map_callback = m.flat_map_callback;
    let outer_parameter = flat_map_callback.parameters()[0];
    let entries = m.map_callback.locals().map_or_default(|locals| locals.entries());
    for (name, _) in entries {
        if flat_map_callback.locals().and_then(|locals| SymbolTable::get(&locals, name)).is_some() {
            return true;
        }
        let captured =
            ctx.checker().resolve_name_exported(name, flat_map_callback, SymbolFlags::All, false);
        if captured.is_some()
            && !ctx.type_parser.references_to_symbol(outer_parameter, captured).is_empty()
        {
            return true;
        }
    }
    false
}

// go: internal/fixables/flat_map_map_to_zip_with.go:128 flatMapMapToZipWithMapCallbackIsTiedToItsFunction fcdce97e32e6c3c3
fn flat_map_map_to_zip_with_map_callback_is_tied_to_its_function(
    ctx: &mut Context<'_, '_>,
    m: &FlatMapMapToZipWithMatch,
) -> bool {
    let map_callback = m.map_callback;
    // A direct eval reads any binding by name at run time, including the
    // function-own ones (arguments, this) that UsesFunctionOwnBindings checks
    // syntactically, so code that may call it cannot move.
    let eval = ctx.checker().get_global_symbol_exported("eval", SymbolFlags::Value, None);
    let tp = &mut ctx.type_parser;
    let mut moved: Vec<Option<P<Node>>> =
        map_callback.parameters().iter().map(|&parameter| Some(parameter)).collect();
    moved.push(map_callback.type_node());
    moved.push(map_callback.body());
    for part in moved {
        let Some(part) = part else {
            continue;
        };
        // The outer callback's own bindings are already excluded by the rule.
        if tp.uses_function_own_bindings(map_callback, part)
            || eval.is_some() && !tp.references_to_symbol(part, eval).is_empty()
        {
            return true;
        }
    }
    false
}

// go: internal/fixables/flat_map_map_to_zip_with.go:145 flatMapMapToZipWithMapCallbackIsZip ca905bfb05873de1
//
// Rust: the parameters' names are set (the rule accepts only plain parameters), as Go assumes; an array literal's
// element list is never nil in tsrs, so that check drops.
fn flat_map_map_to_zip_with_map_callback_is_zip(
    tp: &mut TypeParser<'_>,
    m: &FlatMapMapToZipWithMatch,
) -> bool {
    let map_callback = m.map_callback;
    let pair = m.map_result;
    if map_callback.parameters().len() != 1 {
        return false;
    }
    let zip_would_drop_return_type = map_callback.type_node().is_some();
    if zip_would_drop_return_type {
        return false;
    }
    let outer_parameter = m.flat_map_callback.parameters()[0];
    let map_parameter = map_callback.parameters()[0];
    for parameter in [outer_parameter, map_parameter] {
        if parameter.name().is_none_or(|name| name.kind() != Kind::Identifier)
            || parameter.type_node().is_some()
            || parameter.as_parameter_declaration().question_token().is_some()
        {
            return false;
        }
    }
    if pair.kind() != Kind::ArrayLiteralExpression {
        return false;
    }
    let elements = pair.as_array_literal_expression().elements().nodes();
    if elements.len() != 2 {
        return false;
    }
    for (i, parameter) in [outer_parameter, map_parameter].into_iter().enumerate() {
        let element = elements[i];
        if element.kind() != Kind::Identifier
            || tp.get_symbol_at_location(element) != tp.get_symbol_at_location(parameter.name())
        {
            return false;
        }
    }
    true
}

// go: internal/fixables/flat_map_map_to_zip_with.go:178 flatMapMapToZipWithCombiner 1dd6b3aa0624875e
//
// Rust: Go's tracker is used only to build nodes, so the helper takes the factory (callers pass the tracker).
fn flat_map_map_to_zip_with_combiner(
    tracker: &NodeFactory,
    m: &FlatMapMapToZipWithMatch,
) -> P<Node> {
    let map_callback = m.map_callback;
    let mut parameters =
        vec![tracker.deep_clone_node(Some(m.flat_map_callback.parameters()[0])).unwrap()];
    for &parameter in map_callback.parameters() {
        parameters.push(tracker.deep_clone_node(Some(parameter)).unwrap());
    }
    let mut return_type = None;
    if let Some(original) = map_callback.type_node() {
        return_type = tracker.deep_clone_node(Some(original));
    }
    let body = tracker.deep_clone_node(map_callback.body());
    if map_callback.kind() == Kind::FunctionExpression {
        let mut name = None;
        if let Some(original) = map_callback.name() {
            name = tracker.deep_clone_node(Some(original));
        }
        return tracker.new_function_expression(
            None,
            None,
            name,
            None,
            Some(tracker.new_node_list(parameters)),
            return_type,
            None,
            body,
        );
    }
    tracker.new_arrow_function(
        None,
        None,
        Some(tracker.new_node_list(parameters)),
        return_type,
        None,
        Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
        body,
    )
}
