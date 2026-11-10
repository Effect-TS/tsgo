use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rules::MapMapErrorToMapBothMatch;
use ets_shim::P;
use ets_shim::ast::{self, Kind, Node, NodeFlags, SymbolFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::{ModuleReference, PipingFlowTransformation, TransformationKind};

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_0_EFFECT_1_TRANSFORM_THE_SUCCESS_AND_FAILURE_CHANNELS_ONE_STEP_AT_A_TIME_EFFECT_2_EXPRESSES_BOTH_TRANSFORMATIONS_IN_A_SINGLE_STEP_EFFECT_MAP_MAP_ERROR_TO_MAP_BOTH;

// go: internal/fixables/map_map_error_to_map_both.go:17 MapMapErrorToMapBothFix 4ba34b9a17c2d38e
pub static MAP_MAP_ERROR_TO_MAP_BOTH_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "mapMapErrorToMapBoth",
    description: "Replace with Effect.mapBoth",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["mapMapErrorToMapBoth_fix"],
    run: run_map_map_error_to_map_both_fix,
});

// go: internal/fixables/map_map_error_to_map_both.go:27 runMapMapErrorToMapBothFix 31dda080ebcb5194
fn run_map_map_error_to_map_both_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_map_map_error_to_map_both(&mut ctx.type_parser, Some(source_file)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        // The fix removes one callee and replaces the other, so both must be
        // entity names whose evaluation runs no code.
        if !ast::is_entity_name_expression(m.first.callee())
            || !ast::is_entity_name_expression(m.second.callee())
            || ets_rules::has_explicit_type_arguments(Some(&m.second))
            || map_map_error_path_has_type_arguments(&m.first, &m.second)
            || !map_map_error_keeps_evaluation_order(&m.first, &m.second, m.eager)
        {
            continue;
        }
        let member = effect_module_member(m.replacement_name);
        let target = map_both_reference(ctx, &m, &member);
        let Some(target) = target else {
            continue;
        };
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.".to_string() + m.replacement_name,
            run: Box::new(move |t| {
                let callee = target.expression(t, &member);
                // Keep the callbacks in flow order so they are evaluated as before.
                let (mut first_name, mut second_name) = ("onSuccess", "onFailure");
                if m.first_failure {
                    (first_name, second_name) = (second_name, first_name);
                }
                let options = t.new_object_literal_expression(
                    t.new_node_list(vec![
                        t.new_property_assignment(
                            None,
                            t.new_identifier(first_name),
                            None,
                            None,
                            t.deep_clone_node(Some(*m.first.args().at(0))).unwrap(),
                        ),
                        t.new_property_assignment(
                            None,
                            t.new_identifier(second_name),
                            None,
                            None,
                            t.deep_clone_node(Some(*m.second.args().at(0))).unwrap(),
                        ),
                    ]),
                    true,
                );
                // The first step is the second step's input, so removing it never
                // overlaps the second step's callee or callback.
                if !remove_piping_flow_transformation(t, Some(m.source_file), Some(&m.first)) {
                    return;
                }
                t.replace_node(m.source_file, Some(m.second.callee()), Some(callee), None);
                t.replace_node(m.source_file, Some(*m.second.args().at(0)), Some(options), None);
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}

// go: internal/fixables/map_map_error_to_map_both.go:77 mapBothReference f7460e8f748a8e77
// mapBothReference finds a reference that reaches the replacement API at the
// diagnostic: the file's Effect import, or a binding in scope that is the
// replacement API itself or the Effect module. It returns nil when the file
// has no such binding, for example when the steps only use named imports of
// map and mapError.
//
// Rust: `GetSymbolsInScope` returns one symbol per name (the checker collects them in a symbol table keyed by
// name), so the keys of Go's unstable sort are unique and a stable sort orders them the same way.
fn map_both_reference(
    ctx: &mut Context<'_, '_>,
    m: &MapMapErrorToMapBothMatch,
    member: &ModuleMember,
) -> Option<ModuleMemberReference> {
    let location = m.second.callee();
    if let Some(reference) = imported_module_member(
        ctx,
        location,
        &ets_typeparser::find_effect_module_reference(m.source_file),
        member,
    ) {
        return Some(reference);
    }
    let mut symbols = ctx
        .checker()
        .get_symbols_in_scope_exported(location, SymbolFlags::Value | SymbolFlags::Alias);
    symbols.sort_by(|left, right| left.name().cmp(right.name()));
    let mut module: Option<P<Node>> = None;
    for symbol in symbols {
        if symbol.declarations().len() != 1 || ctx.checker().is_symbol_assigned(symbol) {
            continue;
        }
        let declaration = symbol.declarations()[0];
        if ast::is_part_of_type_only_import_or_export_declaration(declaration) {
            continue;
        }
        let Some(name) = declaration.name() else {
            continue;
        };
        if name.kind() != Kind::Identifier {
            continue;
        }
        let mut reference = name;
        if declaration.kind() == Kind::VariableDeclaration {
            let Some(parent) = declaration.parent() else {
                continue;
            };
            if !parent.flags().contains(NodeFlags::Const)
                || declaration.end().expect_source("a declaration")
                    > location.pos().expect_source("a callee")
            {
                continue;
            }
            let Some(initializer) = declaration.as_variable_declaration().initializer() else {
                continue;
            };
            reference = initializer;
            if ctx.type_parser.get_type_at_location(name)
                != ctx.type_parser.get_type_at_location(reference)
            {
                continue;
            }
        }
        // Prefer a direct binding to the replacement API over a module binding.
        if (member.is_member)(&mut ctx.type_parser, reference) {
            return Some(ModuleMemberReference {
                path: ModuleReference(vec![name.text().to_string()]),
                via_module: false,
            });
        }
        if module.is_none() && (member.is_module)(&mut ctx.type_parser, reference) {
            let name_type = ctx.type_parser.get_type_at_location(name);
            if type_has_member(ctx, name_type, member) {
                module = Some(name);
            }
        }
    }
    if let Some(module) = module {
        return Some(ModuleMemberReference {
            path: ModuleReference(vec![module.text().to_string()]),
            via_module: true,
        });
    }
    None
}

// go: internal/fixables/map_map_error_to_map_both.go:130 mapMapErrorPathHasTypeArguments 03ab74be7e21150c
// mapMapErrorPathHasTypeArguments reports explicit type arguments on any call
// from the first step up to the call that applies the second step: the first
// step itself, its curried application or containing pipe, empty pipe calls in
// between, and the second step's application or containing pipe. Removing the
// first step changes the input type of every one of them, so their type
// arguments would no longer match. It also reports true when the second step's
// application is not an ancestor of the first step.
pub(crate) fn map_map_error_path_has_type_arguments(
    first: &PipingFlowTransformation,
    second: &PipingFlowTransformation,
) -> bool {
    let application = map_map_error_step_application(second);
    let mut node = Some(first.callee());
    while let Some(n) = node {
        if n.kind() == Kind::CallExpression
            && n.as_call_expression()
                .type_arguments()
                .is_some_and(|type_arguments| !type_arguments.nodes().is_empty())
        {
            return true;
        }
        if Some(n) == application {
            return false;
        }
        node = n.parent();
    }
    true
}

// go: internal/fixables/map_map_error_to_map_both.go:147 mapMapErrorStepApplication 43f0caf17c2871b9
// mapMapErrorStepApplication returns the call that applies a step to its input.
pub(crate) fn map_map_error_step_application(step: &PipingFlowTransformation) -> Option<P<Node>> {
    let call = step.callee().parent();
    let Some(call) = call else {
        return call;
    };
    if step.kind() != TransformationKind::Call
        && !ets_rules::is_piping_flow_argument_kind(step.kind())
    {
        return Some(call);
    }
    // A curried step is applied by the call around its factory, and a pipe
    // argument by the pipe or Effect.fn call that holds it.
    let mut application = call.parent();
    while let Some(a) = application
        && a.kind() == Kind::ParenthesizedExpression
    {
        application = a.parent();
    }
    application
}

// go: internal/fixables/map_map_error_to_map_both.go:166 mapMapErrorKeepsEvaluationOrder fd9411ca29a8e818
// mapMapErrorKeepsEvaluationOrder reports whether the fix keeps the observable
// order of evaluation. Steps in one argument list keep it: their callbacks are
// evaluated before any of them runs. Otherwise the fix moves the first callback
// into the second step and the first step's work into the merged step. Only a
// function literal can move across other code, because evaluating it runs none.
pub(crate) fn map_map_error_keeps_evaluation_order(
    first: &PipingFlowTransformation,
    second: &PipingFlowTransformation,
    eager: bool,
) -> bool {
    if map_map_error_share_argument_list(first, second) {
        return true;
    }
    if !is_function_literal(*first.args().at(0))
        && !map_map_error_first_callback_moves_freely(first, second)
    {
        return false;
    }
    // A non-eager step only builds an effect, so moving its work is unobservable.
    !eager || map_map_error_first_work_moves_freely(second)
}

// go: internal/fixables/map_map_error_to_map_both.go:184 mapMapErrorFirstCallbackMovesFreely 1980c3693731a5ea
// mapMapErrorFirstCallbackMovesFreely reports whether nothing observable runs
// between the first callback's evaluation and the second step's callback
// position, apart from applying the first step. That holds when the first step
// evaluates its input before its callback and is the first step of its pipe,
// and the second step evaluates its input before its callback. A curried step
// evaluates its callback before its input, and earlier steps of the first pipe
// run before the second pipe's arguments are evaluated.
pub(crate) fn map_map_error_first_callback_moves_freely(
    first: &PipingFlowTransformation,
    second: &PipingFlowTransformation,
) -> bool {
    if second.kind() != TransformationKind::DataFirst
        && !ets_rules::is_piping_flow_argument_kind(second.kind())
    {
        return false;
    }
    if first.kind() == TransformationKind::DataFirst {
        return true;
    }
    if !ets_rules::is_piping_flow_argument_kind(first.kind()) {
        return false;
    }
    let (index, _) = map_map_error_argument_index(first);
    if first.kind() == TransformationKind::Pipeable {
        return index == 0;
    }
    // Function.pipe and Effect.fn hold their subject or body before the steps.
    index == 1
}

// go: internal/fixables/map_map_error_to_map_both.go:207 mapMapErrorFirstWorkMovesFreely 9e492bacbdf1e53f
// mapMapErrorFirstWorkMovesFreely reports whether an eager first step can run
// in the merged step without crossing observable code. When the first step is
// applied before the second callback is evaluated, the merged step runs that
// work after the second callback and, for a pipe argument, after the arguments
// that follow it.
pub(crate) fn map_map_error_first_work_moves_freely(second: &PipingFlowTransformation) -> bool {
    if second.kind() == TransformationKind::Call || second.kind() == TransformationKind::DataLast {
        true
    } else if second.kind() == TransformationKind::DataFirst {
        is_function_literal(*second.args().at(0))
    } else if ets_rules::is_piping_flow_argument_kind(second.kind()) {
        let (index, count) = map_map_error_argument_index(second);
        is_function_literal(*second.args().at(0)) && index == count - 1
    } else {
        false
    }
}

// go: internal/fixables/map_map_error_to_map_both.go:222 mapMapErrorArgumentIndex 46038525148273f1
// mapMapErrorArgumentIndex returns the position of a pipe argument step in its
// call's argument list and the list's length, or -1 when it is not found.
//
// Rust: tsrs's call arguments are never nil, so that check drops.
pub(crate) fn map_map_error_argument_index(step: &PipingFlowTransformation) -> (i64, i64) {
    let Some(step_call) = step.callee().parent() else {
        return (-1, 0);
    };
    let Some(parent) = step_call.parent() else {
        return (-1, 0);
    };
    if parent.kind() != Kind::CallExpression {
        return (-1, 0);
    }
    let arguments = parent.as_call_expression().arguments().nodes();
    let index = arguments.iter().position(|&argument| argument == step_call);
    (index.map_or(-1, |index| index as i64), arguments.len() as i64)
}

// go: internal/fixables/map_map_error_to_map_both.go:234 isFunctionLiteral 2c4467fc729d82cf
pub(crate) fn is_function_literal(node: P<Node>) -> bool {
    let node = ast::skip_parentheses(node);
    node.kind() == Kind::ArrowFunction || node.kind() == Kind::FunctionExpression
}

// go: internal/fixables/map_map_error_to_map_both.go:239 mapMapErrorShareArgumentList 741876455e1e2da3
pub(crate) fn map_map_error_share_argument_list(
    first: &PipingFlowTransformation,
    second: &PipingFlowTransformation,
) -> bool {
    if !ets_rules::is_piping_flow_argument_kind(first.kind())
        || !ets_rules::is_piping_flow_argument_kind(second.kind())
    {
        return false;
    }
    let (Some(first_step), Some(second_step)) = (first.callee().parent(), second.callee().parent())
    else {
        return false;
    };
    first_step.parent().is_some() && first_step.parent() == second_step.parent()
}
