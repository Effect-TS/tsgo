use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::{NodeOptions, Tracker};
use ets_shim::ast::{self, Kind, Node, NodeFlags, SourceFile};
use ets_shim::core::{P, TextRange, alloc_str};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_shim::scanner;
use ets_typeparser::EffectMajorVersion;

/// Rust-only: the fixable's runtime diagnostic message (Go spells it out in `ErrorCodes`).
static RUNTIME_MESSAGE: &Message = &tsdiag::X_0_IS_CALLED_INSIDE_AN_EFFECT_WITH_A_SEPARATE_RUNTIME_INVOCATION_IN_THIS_CONTEXT_RUN_CHILD_EFFECTS_WITH_THE_SURROUNDING_RUNTIME_WHICH_CAN_BE_ACCESSED_THROUGH_EFFECT_RUNTIME_AND_RUNTIME_1_EFFECT_RUN_EFFECT_INSIDE_EFFECT;

/// Rust-only: the fixable's services diagnostic message (Go spells it out in `ErrorCodes`).
static SERVICES_MESSAGE: &Message = &tsdiag::X_0_IS_CALLED_INSIDE_AN_EFFECT_WITH_A_SEPARATE_SERVICES_INVOCATION_IN_THIS_CONTEXT_CHILD_EFFECTS_RUN_WITH_THE_SURROUNDING_SERVICES_WHICH_CAN_BE_ACCESSED_THROUGH_EFFECT_CONTEXT_AND_EFFECT_1_WITH_EFFECT_RUN_EFFECT_INSIDE_EFFECT;

// go: internal/fixables/run_effect_inside_effect.go:17 RunEffectInsideEffectFix
pub static RUN_EFFECT_INSIDE_EFFECT_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "runEffectInsideEffect",
    description: "Use the current services or a runtime to run the Effect",
    error_codes: vec![RUNTIME_MESSAGE.code(), SERVICES_MESSAGE.code()],
    fix_ids: vec!["runEffectInsideEffect_fix"],
    run: run_run_effect_inside_effect_fix,
});

// go: internal/fixables/run_effect_inside_effect.go:28 runRunEffectInsideEffectFix
//
// Rust: Go's edit closure scans the generator's statements with the type parser before it edits anything. The
// parser is borrowed mutably by the context, which `new_fix_action` borrows, so the scan (which reads only the
// source and the checker, never the tracker) runs just before `new_fix_action`, with the same inputs and order.
// tsrs's variable statements always have a declaration list with a declaration list node, and declarations always
// have a name, so those Go nil checks drop.
fn run_run_effect_inside_effect_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    let supported_effect = ctx.type_parser.supported_effect_version();

    let matches = ets_rules::analyze_run_effect_inside_effect(&mut ctx.type_parser, sf);
    for m in matches {
        if !m.is_nested_scope {
            continue;
        }
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }

        let gen_fn = m.generator_function;
        let block_node = gen_fn.body().unwrap();
        let block = block_node.as_block();

        let mut runtime_identifier = String::new();
        let mut services_identifier = String::new();
        for &stmt in block.statements().nodes() {
            if stmt.kind() != Kind::VariableStatement {
                continue;
            }
            let var_stmt = stmt.as_variable_statement();
            let decl_list = var_stmt.declaration_list().as_variable_declaration_list();
            if decl_list.declarations().nodes().len() != 1 {
                continue;
            }
            let decl = decl_list.declarations().nodes()[0].as_variable_declaration();
            let Some(initializer) = decl.initializer() else {
                continue;
            };
            if initializer.kind() != Kind::YieldExpression {
                continue;
            }
            let yield_expr = initializer.as_yield_expression();
            let Some(yield_expression) = yield_expr.expression() else {
                continue;
            };
            if yield_expr.asterisk_token().is_none()
                || yield_expression.kind() != Kind::CallExpression
            {
                continue;
            }
            let yielded_call = yield_expression.as_call_expression();
            if decl.name().kind() != Kind::Identifier {
                continue;
            }
            let identifier = scanner::get_text_of_node(decl.name());
            if ctx
                .type_parser
                .is_node_reference_to_effect_module_api(yielded_call.expression(), "runtime")
            {
                runtime_identifier.clone_from(&identifier);
            }
            if ctx
                .type_parser
                .is_node_reference_to_effect_module_api(yielded_call.expression(), "context")
            {
                services_identifier = identifier;
            }
        }

        let ctx_ref: &Context<'_, '_> = ctx;
        if let Some(action) = ctx_ref.new_fix_action(FixAction {
            description: run_effect_inside_effect_fix_description(supported_effect).to_string(),
            run: Box::new(move |tracker| {
                let effect_module_identifier = ets_typeparser::find_module_identifier(sf, "Effect");
                if supported_effect == EffectMajorVersion::V4 {
                    if services_identifier.is_empty() {
                        services_identifier = "effectContext".to_string();
                        insert_yielded_effect_module_call(
                            tracker,
                            sf,
                            block_node,
                            &effect_module_identifier,
                            "context",
                            &services_identifier,
                        );
                    }
                } else if runtime_identifier.is_empty() {
                    runtime_identifier = "effectRuntime".to_string();
                    insert_yielded_effect_module_call(
                        tracker,
                        sf,
                        block_node,
                        &effect_module_identifier,
                        "runtime",
                        &runtime_identifier,
                    );
                }

                let runtime_module_identifier =
                    ets_typeparser::find_module_identifier(sf, "Runtime");
                let callee_token_pos = scanner::get_token_pos_of_node(m.callee_node, sf, false);
                let first_arg_pos = m.call_node.as_call_expression().arguments().nodes()[0]
                    .pos()
                    .expect_source("a parsed call argument");
                tracker.delete_range(sf, TextRange::new(callee_token_pos, first_arg_pos));

                let replacement_text = run_effect_inside_effect_replacement_text(
                    supported_effect,
                    &effect_module_identifier,
                    &runtime_module_identifier,
                    m.method_name,
                    &runtime_identifier,
                    &services_identifier,
                );
                tracker.insert_text(
                    sf,
                    ctx_ref.byte_pos_to_lsp_position(first_arg_pos),
                    &replacement_text,
                );
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }

    Vec::new()
}

// go: internal/fixables/run_effect_inside_effect.go:122 runEffectInsideEffectFixDescription
pub(crate) fn run_effect_inside_effect_fix_description(
    supported_effect: EffectMajorVersion,
) -> &'static str {
    if supported_effect == EffectMajorVersion::V4 {
        return "Use the current services to run the Effect";
    }
    "Use a runtime to run the Effect"
}

// go: internal/fixables/run_effect_inside_effect.go:129 runEffectInsideEffectReplacementText
pub(crate) fn run_effect_inside_effect_replacement_text(
    supported_effect: EffectMajorVersion,
    effect_module_identifier: &str,
    runtime_module_identifier: &str,
    method_name: &str,
    runtime_identifier: &str,
    services_identifier: &str,
) -> String {
    if supported_effect == EffectMajorVersion::V4 {
        return format!("{effect_module_identifier}.{method_name}With({services_identifier})(");
    }
    format!("{runtime_module_identifier}.{method_name}({runtime_identifier}, ")
}

// go: internal/fixables/run_effect_inside_effect.go:143 insertYieldedEffectModuleCall
//
// Rust: `block` is the block node (Go: its `*ast.Block`).
pub(crate) fn insert_yielded_effect_module_call(
    tracker: &mut Tracker,
    sf: P<SourceFile>,
    block: P<Node>,
    effect_module_identifier: &str,
    method_name: &str,
    variable_name: &str,
) {
    let effect_id = tracker.new_identifier(alloc_str(effect_module_identifier));
    let method_access = tracker.new_property_access_expression(
        effect_id,
        None,
        tracker.new_identifier(alloc_str(method_name)),
        NodeFlags::None,
    );
    let method_call = tracker.new_call_expression(
        method_access,
        None,
        Some(tracker.new_node_list(vec![tracker.new_keyword_type_node(Kind::NeverKeyword)])),
        tracker.new_node_list(Vec::new()),
        NodeFlags::None,
    );
    let yield_expr = tracker
        .new_yield_expression(Some(tracker.new_token(Kind::AsteriskToken)), Some(method_call));
    let var_decl = tracker.new_variable_declaration(
        tracker.new_identifier(alloc_str(variable_name)),
        None,
        None,
        Some(yield_expr),
    );
    let var_decl_list = tracker
        .new_variable_declaration_list(tracker.new_node_list(vec![var_decl]), NodeFlags::Const);
    let var_stmt = tracker.new_variable_statement(None, var_decl_list);
    ast::set_parent_in_children(var_stmt);

    let insert_pos =
        block.as_block().statements().nodes()[0].pos().expect_source("a parsed statement");
    // Rust: tsrs's `NodeOptions` has crate-private fields, so the literal is a default with Go's two fields set.
    let mut options = NodeOptions::default();
    options.prefix = "\n".to_string();
    options.suffix = "\n".to_string();
    tracker.insert_node_at(sf, insert_pos, var_stmt, options);
}
