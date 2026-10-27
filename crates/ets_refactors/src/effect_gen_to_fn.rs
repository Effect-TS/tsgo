use ets_rewriter::NodeFactoryPaths;
use std::sync::{Arc, LazyLock};

use ets_refactor::{Context, Refactor, RefactorAction};
use ets_rewriter::Tracker;
use ets_shim::ast::{self, Kind, Node, NodeFactory, NodeFlags, SourceFile, TokenFlags};
use ets_shim::astnav;
use ets_shim::core::{P, alloc_str};
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;
use ets_typeparser::{EffectGenCallResult, TypeParser};

use crate::*;

// go: internal/refactors/effect_gen_to_fn.go:13 EffectGenToFn 9a9e78fdcab456dc
pub static EFFECT_GEN_TO_FN: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "effectGenToFn",
    description: "Convert to Effect.fn",
    kind: "rewrite.effect.effectGenToFn",
    run: run_effect_gen_to_fn,
});

// go: internal/refactors/effect_gen_to_fn.go:21 effectGenParsed 70f099194c286700
// effectGenParsed holds the parsed components of a function returning Effect.gen.
pub(crate) struct EffectGenParsed {
    pub(crate) func_node: P<Node>, // The function-like node (arrow, declaration, expression, method)
    pub(crate) gen_call: Arc<EffectGenCallResult>, // The parsed Effect.gen call
    pub(crate) pipe_args: Vec<P<Node>>, // Collected pipe arguments (from .pipe() or pipe() wrappers)
}

// go: internal/refactors/effect_gen_to_fn.go:27 runEffectGenToFn c86548be3f6ff26b
// Rust: tsrs's `get_token_at_position` never returns nil, so Go's nil check drops; the checker is the type
// parser's (`ets_refactor::Context`).
pub(crate) fn run_effect_gen_to_fn(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let token = astnav::get_token_at_position(ctx.source_file, span_pos(ctx));

    // Walk ancestor chain to find a function-like node returning Effect.gen
    let Some(parsed) = find_function_returning_effect_gen(&mut ctx.type_parser, Some(token)) else {
        return Vec::new();
    };

    let fn_name = ets_typeparser::get_function_like_name(parsed.func_node);

    let mut description = "Convert to Effect.fn".to_string();
    if !fn_name.is_empty() {
        description = "Convert to Effect.fn(\"".to_string() + &fn_name + "\")";
    }

    let sf = ctx.source_file;
    let action = ctx.new_refactor_action(RefactorAction {
        description,
        run: Box::new(move |tracker: &mut Tracker| {
            let Some(new_node) = build_effect_gen_to_fn_replacement(tracker, sf, &parsed, &fn_name)
            else {
                return;
            };
            ast::set_parent_in_children(new_node);

            match parsed.func_node.kind() {
                Kind::FunctionDeclaration | Kind::MethodDeclaration => {
                    tracker.replace_node(sf, Some(parsed.func_node), Some(new_node), None);
                }
                _ => {
                    replace_expression_via_statement(tracker, sf, parsed.func_node, new_node);
                }
            }
        }),
    });
    let Some(mut action) = action else {
        return Vec::new();
    };

    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.effectGenToFn");
    vec![action]
}

// go: internal/refactors/effect_gen_to_fn.go:76 findFunctionReturningEffectGen 11f75db6d99deacb
// findFunctionReturningEffectGen walks from the token up the ancestor chain to find
// a function-like node whose body directly returns an Effect.gen call, optionally
// wrapped in pipe/pipeable chains.
//
// Rust: Go's checker parameter is the parser's (`tp.checker()`).
pub(crate) fn find_function_returning_effect_gen(
    tp: &mut TypeParser<'_>,
    token: Option<P<Node>>,
) -> Option<EffectGenParsed> {
    let mut node = token;
    while let Some(n) = node {
        match n.kind() {
            Kind::ArrowFunction | Kind::FunctionDeclaration | Kind::MethodDeclaration => {
                // Hit a function boundary without finding Effect.gen — stop
                return parse_function_returning_effect_gen(tp, n);
            }
            Kind::FunctionExpression => {
                // Don't match on generator functions (those are inside Effect.gen already)
                let fe = n.as_function_expression();
                if fe.asterisk_token().is_none() {
                    return parse_function_returning_effect_gen(tp, n);
                }
            }
            _ => {}
        }
        node = n.parent();
    }
    None
}

// go: internal/refactors/effect_gen_to_fn.go:104 parseFunctionReturningEffectGen e91506fcc6bce83f
// parseFunctionReturningEffectGen extracts the Effect.gen call and any pipe args
// from a function-like node. Returns nil if the function doesn't match.
//
// Rust: Go's unused checker parameter drops (see `find_function_returning_effect_gen`).
pub(crate) fn parse_function_returning_effect_gen(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
) -> Option<EffectGenParsed> {
    let body = ets_typeparser::get_function_like_body(node)?;

    // Skip through { return <expr> } blocks to get the actual expression
    let mut subject = skip_return_block(body);

    // Peel off pipe wrappers, collecting args
    let mut pipe_args: Vec<P<Node>> = Vec::new();
    while let Some(pipe_result) = tp.parse_pipe_call(subject) {
        // Prepend args (inner pipes go first)
        let mut args = pipe_result.args().copy().unwrap_or_default();
        args.extend(pipe_args);
        pipe_args = args;
        subject = pipe_result.subject();
    }

    // Check if the core expression is Effect.gen(function*() { ... })
    let gen_call = tp.effect_gen_call(subject)?;

    Some(EffectGenParsed { func_node: node, gen_call, pipe_args })
}

// go: internal/refactors/effect_gen_to_fn.go:139 skipReturnBlock 2ad5b59914ffe1b6
// skipReturnBlock unwraps a block containing a single return statement to get the expression.
pub(crate) fn skip_return_block(node: P<Node>) -> P<Node> {
    if node.kind() != Kind::Block {
        return node;
    }
    let block = node.as_block();
    if block.statements().nodes().len() != 1 {
        return node;
    }
    let stmt = block.statements().nodes()[0];
    if stmt.kind() != Kind::ReturnStatement {
        return node;
    }
    let ret = stmt.as_return_statement();
    let Some(expression) = ret.expression() else {
        return node;
    };
    expression
}

// go: internal/refactors/effect_gen_to_fn.go:159 buildEffectGenToFnReplacement a02dd96461af0df0
// buildEffectGenToFnReplacement builds the replacement node for the effectGenToFn refactor.
pub(crate) fn build_effect_gen_to_fn_replacement(
    tracker: &NodeFactory,
    sf: P<SourceFile>,
    parsed: &EffectGenParsed,
    fn_name: &str,
) -> Option<P<Node>> {
    let node = parsed.func_node;
    let gen_call = &parsed.gen_call;

    // Get the generator body
    let mut block_body = None;
    if let Some(body) = gen_call.body() {
        if body.kind() == Kind::Block {
            block_body = tracker.deep_clone_node(Some(body));
        } else {
            // Expression body: wrap in { return <expr> }
            let cloned_expr = tracker.deep_clone_node(Some(body));
            let return_stmt = tracker.new_return_statement(cloned_expr);
            block_body = Some(tracker.new_block(tracker.new_node_list(vec![return_stmt]), true));
        }
    }
    let block_body = block_body?;

    // Get type parameters and parameters from the outer function
    let mut type_params = None;
    let mut params = None;

    match node.kind() {
        Kind::ArrowFunction => {
            let af = node.as_arrow_function();
            type_params = clone_node_list(tracker, af.type_parameters());
            params = clone_node_list(tracker, af.parameters());
        }
        Kind::FunctionDeclaration => {
            let fd = node.as_function_declaration();
            type_params = clone_node_list(tracker, fd.type_parameters());
            params = clone_node_list(tracker, fd.parameters());
        }
        Kind::FunctionExpression => {
            let fe = node.as_function_expression();
            type_params = clone_node_list(tracker, fe.type_parameters());
            params = clone_node_list(tracker, fe.parameters());
        }
        Kind::MethodDeclaration => {
            let md = node.as_method_declaration();
            type_params = clone_node_list(tracker, md.type_parameters());
            params = clone_node_list(tracker, md.parameters());
        }
        _ => {}
    }

    // Build function*(params) { body }
    let gen_fn = tracker.new_function_expression(
        None,                                         // modifiers
        Some(tracker.new_token(Kind::AsteriskToken)), // asterisk (generator)
        None,                                         // name
        type_params,                                  // typeParameters
        params,                                       // parameters
        None,                                         // returnType
        None,                                         // fullSignature
        Some(block_body),
    );

    // Build Effect.fn or Effect.fn("name")
    let fn_access = tracker.new_property_access_path(
        &ets_typeparser::find_effect_module_reference(sf).append(&["fn"]),
    );

    let fn_call_expr = if !fn_name.is_empty() {
        // Effect.fn("name")
        tracker.new_call_expression(
            fn_access,
            None,
            None,
            tracker.new_node_list(vec![
                tracker.new_string_literal(alloc_str(fn_name), TokenFlags::None),
            ]),
            NodeFlags::None,
        )
    } else {
        fn_access
    };

    // Build the arguments: [generatorFn, ...pipeArgs]
    let mut args = vec![gen_fn];
    for &pipe_arg in &parsed.pipe_args {
        args.push(tracker.deep_clone_node(Some(pipe_arg)).unwrap());
    }

    // Effect.fn("name")(function*(params) { body }, ...pipeArgs)
    let effect_fn_call = tracker.new_call_expression(
        fn_call_expr,
        None,
        None,
        tracker.new_node_list(args),
        NodeFlags::None,
    );

    // Wrap in appropriate declaration
    Some(build_effect_gen_to_fn_declaration(tracker, node, effect_fn_call))
}

// go: internal/refactors/effect_gen_to_fn.go:248 buildEffectGenToFnDeclaration 08f0710450bf746f
// buildEffectGenToFnDeclaration wraps the Effect.fn call in the appropriate declaration.
pub(crate) fn build_effect_gen_to_fn_declaration(
    tracker: &NodeFactory,
    node: P<Node>,
    effect_fn_call: P<Node>,
) -> P<Node> {
    match node.kind() {
        Kind::FunctionDeclaration => {
            let fd = node.as_function_declaration();
            let Some(name) = fd.name() else {
                return effect_fn_call;
            };

            // Collect modifiers
            let mut mods = None;
            if let Some(modifiers) = fd.modifiers() {
                let mut mod_nodes = Vec::new();
                for &m in modifiers.nodes() {
                    mod_nodes.push(tracker.new_modifier(m.kind()));
                }
                if !mod_nodes.is_empty() {
                    mods = Some(tracker.new_modifier_list(mod_nodes));
                }
            }

            // const name = Effect.fn("name")(...)
            let var_decl = tracker.new_variable_declaration(
                tracker.deep_clone_node(Some(name)).unwrap(),
                None, // exclamationToken
                None, // type
                Some(effect_fn_call),
            );
            let var_decl_list = tracker.new_variable_declaration_list(
                tracker.new_node_list(vec![var_decl]),
                NodeFlags::Const,
            );
            tracker.new_variable_statement(mods, var_decl_list)
        }

        Kind::MethodDeclaration => {
            let md = node.as_method_declaration();
            // Convert method to property declaration: name = Effect.fn("name")(...)
            let mut mods = None;
            if let Some(modifiers) = md.modifiers() {
                let mut mod_nodes = Vec::new();
                for &m in modifiers.nodes() {
                    mod_nodes.push(tracker.new_modifier(m.kind()));
                }
                if !mod_nodes.is_empty() {
                    mods = Some(tracker.new_modifier_list(mod_nodes));
                }
            }
            tracker.new_property_declaration(
                mods,
                tracker.deep_clone_node(Some(md.name())).unwrap(),
                None, // postfixToken
                None, // type
                Some(effect_fn_call),
            )
        }

        // For arrow functions and function expressions, return the expression directly
        _ => effect_fn_call,
    }
}
