use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::TypeParser;

// go: internal/rules/overridden_schema_constructor.go:14 OverriddenSchemaConstructor 5fb57691a3481e54
pub static OVERRIDDEN_SCHEMA_CONSTRUCTOR: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "overriddenSchemaConstructor",
        group: "correctness",
        description: "Prevents overriding constructors in Schema classes which breaks decoding behavior",
        default_severity: Severity::Error,
        supported_effect: vec!["v3", "v4"],
        codes: vec![tsdiag::THIS_SCHEMA_SUBCLASS_DEFINES_ITS_OWN_CONSTRUCTOR_FOR_SCHEMA_CLASSES_CONSTRUCTOR_OVERRIDES_BREAK_DECODING_BEHAVIOR_FOR_THE_CLASS_SHAPE_CUSTOM_CONSTRUCTION_CAN_BE_EXPRESSED_THROUGH_A_STATIC_NEW_METHOD_INSTEAD_EFFECT_OVERRIDDEN_SCHEMA_CONSTRUCTOR.code()],
        run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
            let matches = analyze_overridden_schema_constructor(ctx.type_parser, ctx.source_file);
            let mut diags = Vec::with_capacity(matches.len());
            for m in &matches {
                diags.push(ctx.new_diagnostic(m.source_file, m.location, &tsdiag::THIS_SCHEMA_SUBCLASS_DEFINES_ITS_OWN_CONSTRUCTOR_FOR_SCHEMA_CLASSES_CONSTRUCTOR_OVERRIDES_BREAK_DECODING_BEHAVIOR_FOR_THE_CLASS_SHAPE_CUSTOM_CONSTRUCTION_CAN_BE_EXPRESSED_THROUGH_A_STATIC_NEW_METHOD_INSTEAD_EFFECT_OVERRIDDEN_SCHEMA_CONSTRUCTOR, &[], &[]));
            }
            diags
        },
    }
});

// go: internal/rules/overridden_schema_constructor.go:33 OverriddenSchemaConstructorMatch 9b518efd777f5c66
// OverriddenSchemaConstructorMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fixes for the overriddenSchemaConstructor pattern.
#[derive(Clone)]
pub struct OverriddenSchemaConstructorMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,       // The error range for the constructor node
    pub constructor_node: P<Node>, // The constructor declaration AST node
    pub has_body: bool, // Whether the constructor has a body (the static fix is only available when true)
}

// go: internal/rules/overridden_schema_constructor.go:42 AnalyzeOverriddenSchemaConstructor 6a1c3ffff59a0cc3
// AnalyzeOverriddenSchemaConstructor finds all class declarations extending Schema
// that have an overridden constructor which is not an allowed passthrough pattern.
//
// Rust: Go's (unused) checker argument is dropped.
pub fn analyze_overridden_schema_constructor(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<OverriddenSchemaConstructorMatch> {
    let mut matches = Vec::new();

    let mut node_to_visit: Vec<P<Node>> = Vec::new();
    sf.as_node().for_each_child(&mut |child| {
        node_to_visit.push(child);
        false
    });

    while let Some(node) = node_to_visit.pop() {
        if node.kind() == Kind::ClassDeclaration
            && let Some(m) = check_overridden_schema_constructor(tp, sf, node)
        {
            matches.push(m);
        }

        node.for_each_child(&mut |child| {
            node_to_visit.push(child);
            false
        });
    }

    matches
}

// go: internal/rules/overridden_schema_constructor.go:68 checkOverriddenSchemaConstructor 361d8769e4883e1c
//
// Rust: Go's (ignored) checker argument is dropped.
pub(crate) fn check_overridden_schema_constructor(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Option<OverriddenSchemaConstructorMatch> {
    let extends_elements = ast::get_extends_heritage_clause_elements(node);
    if extends_elements.is_empty() {
        return None;
    }

    if node.kind() != Kind::ClassDeclaration {
        return None;
    }

    let mut extends_schema = false;
    for &elem in extends_elements {
        if elem.kind() != Kind::ExpressionWithTypeArguments {
            continue;
        }
        let expr = elem.as_expression_with_type_arguments().expression;
        let t = tp.get_type_at_location(expr);
        if let Some(t) = t
            && tp.is_schema_type(t)
        {
            extends_schema = true;
            break;
        }
    }

    if !extends_schema {
        return None;
    }

    for &member in node.members() {
        if member.kind() == Kind::Constructor {
            if is_allowed_constructor(member) {
                continue;
            }
            let has_body = member.body().is_some_and(|body| body.kind() == Kind::Block);
            return Some(OverriddenSchemaConstructorMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, member),
                constructor_node: member,
                has_body,
            });
        }
    }

    None
}

// go: internal/rules/overridden_schema_constructor.go:120 isAllowedConstructor d144bcab2e4795ea
// isAllowedConstructor checks if a constructor is a passthrough that simply forwards
// exactly 2 parameters to super(). This pattern is used internally by Schema and is allowed.
pub(crate) fn is_allowed_constructor(ctor_node: P<Node>) -> bool {
    let Some(body) = ctor_node.body() else {
        return false;
    };
    if body.kind() != Kind::Block {
        return false;
    }
    let statements = body.statements();
    if statements.len() != 1 {
        return false;
    }

    let stmt = statements[0];
    if stmt.kind() != Kind::ExpressionStatement {
        return false;
    }

    let expr = stmt.as_expression_statement().expression;
    if expr.kind() != Kind::CallExpression {
        return false;
    }

    let call = expr.as_call_expression();
    if call.expression.kind() != Kind::SuperKeyword {
        return false;
    }

    // Constructor must have exactly 2 parameters
    let parameters = ctor_node.parameters();
    if parameters.len() != 2 {
        return false;
    }

    // Collect parameter names (must all be simple identifiers)
    let mut expected_names = Vec::with_capacity(2);
    for &param in parameters {
        let Some(name) = param.name() else {
            return false;
        };
        if name.kind() != Kind::Identifier {
            return false;
        }
        expected_names.push(ets_shim::scanner::get_text_of_node(name));
    }

    // super() call must have exactly 2 arguments matching parameter names in order
    let arguments = call.arguments.nodes();
    if arguments.len() != 2 {
        return false;
    }

    for (i, &arg) in arguments.iter().enumerate() {
        if arg.kind() != Kind::Identifier {
            return false;
        }
        if ets_shim::scanner::get_text_of_node(arg) != expected_names[i] {
            return false;
        }
    }

    true
}
