use std::cell::Cell;
use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::Tracker;
use ets_shim::P;
use ets_shim::ast::{self, Kind, Node, NodeFlags, NodeList, SourceFile, SymbolFlags};
use ets_shim::checker::Type;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::{
    EffectMajorVersion, LazyExpressionFlags, ParsedLazyExpression, PipingFlow, PipingFlowSubject,
    PipingFlowTransformation, TypeParser, parse_lazy_expression,
};

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_EFFECT_CODE_RACES_PROMISES_WITH_PROMISE_0_THE_LOSING_PROMISES_KEEP_RUNNING_AND_ARE_NOT_INTERRUPTED_USE_EFFECT_1_OVER_EFFECT_VALUES_INSTEAD_EFFECT_PROMISE_RACE_IN_EFFECT;

// go: internal/fixables/promise_race_in_effect.go:14 PromiseRaceInEffectFix 882937f3c513a7b0
pub static PROMISE_RACE_IN_EFFECT_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "promiseRaceInEffect",
    description: "Race Effect values with Effect.raceAll or Effect.raceAllFirst instead of racing promises",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["promiseRaceInEffect_fix"],
    run: run_promise_race_in_effect_fix,
});

// go: internal/fixables/promise_race_in_effect.go:24 promiseRaceInEffectFixPlan fbc205082096bc2e
pub(crate) struct PromiseRaceInEffectFixPlan {
    pub(crate) outer: PipingFlow,
    pub(crate) thunk: P<Node>,
    pub(crate) callback: ParsedLazyExpression,
    pub(crate) receiver: P<Node>,
    pub(crate) map_name: P<Node>,
    pub(crate) race: ModuleMember,
    pub(crate) race_ref: ModuleMemberReference,
    pub(crate) constructor: ModuleMember,
    pub(crate) ctor_ref: ModuleMemberReference,
}

// go: internal/fixables/promise_race_in_effect.go:36 runPromiseRaceInEffectFix 435f5bd290e8a4c2
//
// Rust: the fix action's closure moves the plan (its `moduleMember`s are boxed and not `Clone`).
fn run_promise_race_in_effect_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let matches = ets_rules::analyze_promise_race_in_effect(&mut ctx.type_parser, ctx.source_file);
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        let Some(plan) = promise_race_in_effect_plan(ctx, &m) else {
            continue;
        };
        let source_file = m.source_file;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.".to_string() + plan.race.name,
            run: Box::new(move |tracker| {
                let replacement = promise_race_in_effect_replacement(tracker, &plan);
                tracker.replace_piping_flow_prefix(
                    Some(source_file),
                    Some(&plan.outer),
                    1,
                    Some(replacement),
                );
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}

// go: internal/fixables/promise_race_in_effect.go:57 promiseRaceInEffectPlan 50df43b078064579
//
// Rust: the subject predicate stores the parsed callback in a `Cell` (Go's closure assigns a captured variable).
pub(crate) fn promise_race_in_effect_plan(
    ctx: &mut Context<'_, '_>,
    m: &ets_rules::PromiseRaceInEffectMatch,
) -> Option<PromiseRaceInEffectFixPlan> {
    // Effect v4 raceAll never settles on an empty list, while Promise.any([])
    // rejects, so only v3 any and v4 race have a matching replacement.
    let race = if m.method == "race"
        && ctx.type_parser.supported_effect_version() == EffectMajorVersion::V4
    {
        "raceAllFirst"
    } else if m.method == "any"
        && ctx.type_parser.supported_effect_version() == EffectMajorVersion::V3
    {
        "raceAll"
    } else {
        return None;
    };

    let callback: Cell<Option<ParsedLazyExpression>> = Cell::new(None);
    if m.index != 1
        || !PipingFlow::matches_exactly(
            Some(&m.flow),
            &mut ctx.type_parser,
            &|_: &mut TypeParser<'_>, subject: &PipingFlowSubject| {
                callback.set(parse_lazy_expression(
                    subject.node(),
                    LazyExpressionFlags::AllowMultiStatementBlock,
                ));
                callback.get().is_some()
            },
            &[
                &|tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| {
                    promise_race_in_effect_is_untyped_native_map_step(tp, step)
                },
                &|_: &mut TypeParser<'_>, step: &PipingFlowTransformation| {
                    promise_race_in_effect_is_bare_promise_member_step(step)
                },
            ],
        )
    {
        return None;
    }
    let callback = callback.get()?;
    let map_callee = m.flow.transformations().at(0).callee();

    if !{
        let list_type =
            PipingFlow::transformation_input_type(Some(&m.flow), &mut ctx.type_parser, 1);
        promise_race_in_effect_items_are_promises(&mut ctx.type_parser, list_type)
    } || promise_race_in_effect_is_callback_name_referenced(&mut ctx.type_parser, &callback)
        || !promise_race_in_effect_parameters_bind_without_running_code(
            callback.node().parameters(),
        )
    {
        return None;
    }

    let thunk = promise_race_in_effect_thunk(m.flow.node())?;

    let (outer, constructor) =
        promise_race_in_effect_constructor_flow(&mut ctx.type_parser, m.source_file, thunk);
    let outer = outer?;

    let receiver = map_callee.as_property_access_expression().expression();
    if !ctx.type_parser.is_expression_value_stable_at_location(receiver, outer.node())
        || !promise_race_in_effect_thunk_parameters_stay_in_body(
            ctx,
            thunk,
            m.flow.node(),
            callback.body(),
        )
    {
        return None;
    }

    let effect_module = ets_typeparser::find_effect_module_reference(Some(m.source_file));
    let race_member = effect_module_member(race);
    let race_ref = resolve_module_member(ctx, outer.node(), &effect_module, &race_member);
    let constructor_member = effect_module_member(constructor);
    let ctor_ref = resolve_module_member(ctx, callback.body(), &effect_module, &constructor_member);
    let (Some(race_ref), Some(ctor_ref)) = (race_ref, ctor_ref) else {
        return None;
    };

    Some(PromiseRaceInEffectFixPlan {
        outer,
        thunk,
        callback,
        receiver,
        map_name: map_callee.as_property_access_expression().name(),
        race: race_member,
        race_ref,
        constructor: constructor_member,
        ctor_ref,
    })
}

// go: internal/fixables/promise_race_in_effect.go:130 promiseRaceInEffectIsUntypedNativeMapStep 9f530a5de822b1f7
pub(crate) fn promise_race_in_effect_is_untyped_native_map_step(
    tp: &mut TypeParser<'_>,
    step: &PipingFlowTransformation,
) -> bool {
    let callee = step.callee();
    step.args().len() == 0
        && step.type_arguments().is_none()
        && ast::is_property_access_expression(callee)
        && tp.is_native_array_method_reference(callee, "map")
}

// go: internal/fixables/promise_race_in_effect.go:136 promiseRaceInEffectIsBarePromiseMemberStep 3f3830bd710fb7fb
pub(crate) fn promise_race_in_effect_is_bare_promise_member_step(
    step: &PipingFlowTransformation,
) -> bool {
    let callee = step.callee();
    ast::is_property_access_expression(callee)
        && ast::is_identifier(callee.as_property_access_expression().expression())
}

// go: internal/fixables/promise_race_in_effect.go:141 promiseRaceInEffectItemsArePromises bd87d97aea71b894
pub(crate) fn promise_race_in_effect_items_are_promises(
    tp: &mut TypeParser<'_>,
    list_type: Option<P<Type>>,
) -> bool {
    let Some(item_type) = tp.array_type(list_type) else {
        return false;
    };
    for member in tp.unroll_union_members(item_type).unwrap_or_default() {
        if tp.promise_type(member).is_none() {
            return false;
        }
    }
    true
}

// go: internal/fixables/promise_race_in_effect.go:154 promiseRaceInEffectIsCallbackNameReferenced 612e169526b8e6cb
pub(crate) fn promise_race_in_effect_is_callback_name_referenced(
    tp: &mut TypeParser<'_>,
    callback: &ParsedLazyExpression,
) -> bool {
    let Some(name) = callback.node().name() else {
        return false;
    };
    let symbol = tp.get_symbol_at_location(name);
    for reference in tp.references_to_symbol(callback.node(), symbol) {
        if reference != name {
            return true;
        }
    }
    false
}

// go: internal/fixables/promise_race_in_effect.go:167 promiseRaceInEffectParametersBindWithoutRunningCode e13410cbd5aa3563
pub(crate) fn promise_race_in_effect_parameters_bind_without_running_code(
    parameters: &[P<Node>],
) -> bool {
    for parameter in parameters {
        match parameter.name() {
            Some(name) if name.kind() == Kind::Identifier && parameter.initializer().is_none() => {}
            _ => return false,
        }
    }
    true
}

// go: internal/fixables/promise_race_in_effect.go:176 promiseRaceInEffectThunk d3838406bc457e89
pub(crate) fn promise_race_in_effect_thunk(race_call: P<Node>) -> Option<P<Node>> {
    let mut thunk_child = race_call;
    while let Some(parent) = thunk_child.parent()
        && parent.kind() == Kind::ParenthesizedExpression
    {
        thunk_child = parent;
    }
    let thunk = thunk_child.parent()?;
    if thunk.kind() != Kind::ArrowFunction
        || parse_lazy_expression(thunk, LazyExpressionFlags::None).is_none()
    {
        return None;
    }
    let body = thunk.body().expect("an arrow function has a body");
    if body.kind() == Kind::Block || ast::skip_parentheses(body) != race_call {
        return None;
    }
    Some(thunk)
}

// go: internal/fixables/promise_race_in_effect.go:190 promiseRaceInEffectConstructorFlow 0d0c6c1a46eababb
//
// Rust: the step predicate stores the constructor name in a `Cell` (Go's closure assigns a captured variable).
pub(crate) fn promise_race_in_effect_constructor_flow(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    thunk: P<Node>,
) -> (Option<PipingFlow>, &'static str) {
    let constructor = Cell::new("");
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        if PipingFlow::matches_prefix(
            Some(flow),
            tp,
            &|_: &mut TypeParser<'_>, subject: &PipingFlowSubject| subject.node() == Some(thunk),
            &[&|tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| {
                if step.args().len() != 0 {
                    return false;
                }
                for name in ["promise", "tryPromise"] {
                    if tp.is_node_reference_to_effect_module_api(step.callee(), name) {
                        constructor.set(name);
                        return true;
                    }
                }
                false
            }],
        ) {
            return (Some(flow.clone()), constructor.get());
        }
    }
    (None, "")
}

// go: internal/fixables/promise_race_in_effect.go:214 promiseRaceInEffectThunkParametersStayInBody a1fdfdbb377924a1
pub(crate) fn promise_race_in_effect_thunk_parameters_stay_in_body(
    ctx: &mut Context<'_, '_>,
    thunk: P<Node>,
    race_call: P<Node>,
    body: P<Node>,
) -> bool {
    for &parameter in thunk.parameters() {
        let Some(name) = parameter.name() else {
            return false;
        };
        if name.kind() != Kind::Identifier {
            return false;
        }
        let symbol = ctx.type_parser.get_symbol_at_location(name);
        let Some(symbol) = symbol else {
            return false;
        };
        if ctx.checker().resolve_name_exported(name.text(), body, SymbolFlags::Value, false)
            != Some(symbol)
        {
            return false;
        }
        for reference in ctx.type_parser.references_to_symbol(race_call, symbol) {
            if reference.pos() < body.pos() || reference.end() > body.end() {
                return false;
            }
        }
    }
    true
}

// go: internal/fixables/promise_race_in_effect.go:234 promiseRaceInEffectReplacement ce2736c47fa6921d
pub(crate) fn promise_race_in_effect_replacement(
    tracker: &Tracker,
    plan: &PromiseRaceInEffectFixPlan,
) -> P<Node> {
    let mut return_type = None;
    if let Some(original) = plan.callback.node().type_node() {
        return_type = tracker.deep_clone_node(Some(original));
    }
    let per_item_thunk = tracker.new_arrow_function(
        None,
        None,
        Some(promise_race_in_effect_clone_nodes(tracker, plan.thunk.parameters())),
        return_type,
        None,
        Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
        tracker.deep_clone_node(Some(plan.callback.body())),
    );
    let item = tracker.new_call_expression(
        plan.ctor_ref.expression(tracker, &plan.constructor),
        None,
        None,
        tracker.new_node_list(vec![per_item_thunk]),
        NodeFlags::None,
    );
    let parameters = promise_race_in_effect_clone_nodes(tracker, plan.callback.node().parameters());
    let callback = if plan.callback.node().kind() == Kind::FunctionExpression {
        let mut name = None;
        if let Some(original) = plan.callback.node().name() {
            name = tracker.deep_clone_node(Some(original));
        }
        tracker.new_function_expression(
            None,
            None,
            name,
            None,
            Some(parameters),
            None,
            None,
            Some(tracker.new_block(
                tracker.new_node_list(vec![tracker.new_return_statement(Some(item))]),
                true,
            )),
        )
    } else {
        tracker.new_arrow_function(
            None,
            None,
            Some(parameters),
            None,
            None,
            Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
            Some(item),
        )
    };
    let map_call = tracker.new_call_expression(
        tracker.new_property_access_expression(
            tracker.deep_clone_node(Some(plan.receiver)).unwrap(),
            None,
            tracker.deep_clone_node(Some(plan.map_name)).unwrap(),
            NodeFlags::None,
        ),
        None,
        None,
        tracker.new_node_list(vec![callback]),
        NodeFlags::None,
    );
    tracker.new_call_expression(
        plan.race_ref.expression(tracker, &plan.race),
        None,
        None,
        tracker.new_node_list(vec![map_call]),
        NodeFlags::None,
    )
}

// go: internal/fixables/promise_race_in_effect.go:291 promiseRaceInEffectCloneNodes 6a04ebb4de02a5d8
pub(crate) fn promise_race_in_effect_clone_nodes(
    tracker: &Tracker,
    nodes: &[P<Node>],
) -> P<NodeList> {
    let mut clones = Vec::with_capacity(nodes.len());
    for &node in nodes {
        clones.push(tracker.deep_clone_node(Some(node)).unwrap());
    }
    tracker.new_node_list(clones)
}
