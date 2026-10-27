use std::rc::Rc;
use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::{self, Kind, Node, NodeFlags, NodeVisitor, NodeVisitorHooks};
use ets_shim::core::P;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_SCHEMA_SUBCLASS_DEFINES_ITS_OWN_CONSTRUCTOR_FOR_SCHEMA_CLASSES_CONSTRUCTOR_OVERRIDES_BREAK_DECODING_BEHAVIOR_FOR_THE_CLASS_SHAPE_CUSTOM_CONSTRUCTION_CAN_BE_EXPRESSED_THROUGH_A_STATIC_NEW_METHOD_INSTEAD_EFFECT_OVERRIDDEN_SCHEMA_CONSTRUCTOR;

// go: internal/fixables/overridden_schema_constructor.go:12 OverriddenSchemaConstructorFix cff5580ca89382f2
pub static OVERRIDDEN_SCHEMA_CONSTRUCTOR_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "overriddenSchemaConstructorFix",
    description: "Remove or rewrite constructor in Schema class",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["overriddenSchemaConstructor_fix", "overriddenSchemaConstructor_static"],
    run: run_overridden_schema_constructor_fix,
});

// go: internal/fixables/overridden_schema_constructor.go:23 runOverriddenSchemaConstructorFix df1b8e415f0c21fa
fn run_overridden_schema_constructor_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_overridden_schema_constructor(&mut ctx.type_parser, sf);
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }

        let mut actions = Vec::new();

        // _static action: Rewrite using the static 'new' pattern (only when body exists)
        if m.has_body && constructor_supports_static_rewrite(m.constructor_node) {
            let constructor_node = m.constructor_node;
            if let Some(action) = ctx.new_fix_action(FixAction {
                description: "Rewrite using the static 'new' pattern".to_string(),
                run: Box::new(move |tracker| {
                    let ctor = constructor_node.as_constructor_declaration();

                    // Build a visitor that transforms super(...) calls and this keywords
                    //
                    // Rust: the visitor callback is a `'static` closure: it holds a clone of the tracker's
                    // factory (Go's closure reads the tracker) and receives the visitor (Go's `v`).
                    let f = tracker.node_factory.clone();
                    let visit_fn = move |v: &mut NodeVisitor, node: P<Node>| -> Option<P<Node>> {
                        // Replace super(...) expression statements with: const _this = new this(...)
                        if node.kind() == Kind::ExpressionStatement {
                            let expr = node.as_expression_statement().expression();
                            if expr.kind() == Kind::CallExpression {
                                let call = expr.as_call_expression();
                                if call.expression().kind() == Kind::SuperKeyword {
                                    let construct_this = f.new_new_expression(
                                        f.new_identifier("this"),
                                        None,
                                        Some(call.arguments()),
                                    );
                                    return Some(f.new_variable_statement(
                                        None,
                                        f.new_variable_declaration_list(
                                            f.new_node_list(vec![f.new_variable_declaration(
                                                f.new_identifier("_this"),
                                                None,
                                                None,
                                                Some(construct_this),
                                            )]),
                                            NodeFlags::Const,
                                        ),
                                    ));
                                }
                            }
                        }
                        // Replace this keywords with _this identifier
                        if node.kind() == Kind::ThisKeyword {
                            return Some(f.new_identifier("_this"));
                        }
                        v.visit_each_child(Some(node))
                    };
                    let mut v = ast::new_node_visitor(
                        Some(Rc::new(visit_fn)),
                        Some(tracker.node_factory.clone()),
                        NodeVisitorHooks::default(),
                    );

                    // Transform the constructor body
                    let new_body = v.visit_node(ctor.body()).unwrap();

                    // Append return _this to the transformed body's statements
                    let body_block = new_body.as_block();
                    let mut new_statements = body_block.statements().nodes().to_vec();
                    new_statements
                        .push(tracker.new_return_statement(Some(tracker.new_identifier("_this"))));
                    let new_block = tracker.new_block(tracker.new_node_list(new_statements), true);

                    // Build modifier list with public and static
                    let modifiers = tracker.new_modifier_list(vec![
                        tracker.new_modifier(Kind::PublicKeyword),
                        tracker.new_modifier(Kind::StaticKeyword),
                    ]);

                    // Build the replacement method declaration
                    let new_method = tracker.new_method_declaration(
                        Some(modifiers),
                        None,
                        tracker.new_identifier("new"),
                        None,
                        ctor.type_parameters(),
                        ctor.parameters(),
                        ctor.type_(),
                        None,
                        Some(new_block),
                    );

                    tracker.replace_node(sf, Some(constructor_node), Some(new_method), None);
                }),
            }) {
                actions.push(action);
            }
        }

        // _fix action: Remove the constructor override (always available)
        let constructor_node = m.constructor_node;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Remove the constructor override".to_string(),
            run: Box::new(move |tracker| {
                tracker.delete(sf, constructor_node);
            }),
        }) {
            actions.push(action);
        }

        return actions;
    }

    Vec::new()
}

// go: internal/fixables/overridden_schema_constructor.go:134 constructorSupportsStaticRewrite 5f47205b2587306f
pub(crate) fn constructor_supports_static_rewrite(ctor_node: P<Node>) -> bool {
    let ctor = ctor_node.as_constructor_declaration();
    let Some(parameters) = ctor.parameters() else {
        return true;
    };
    for &param_node in parameters.nodes() {
        if ast::is_parameter_property_declaration(param_node, ctor_node) {
            return false;
        }
    }
    true
}
