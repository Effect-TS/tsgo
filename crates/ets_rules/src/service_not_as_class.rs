use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, ModifierList, Node, NodeFlags, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectMajorVersion, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::CONTEXT_SERVICE_IS_ASSIGNED_TO_A_VARIABLE_HERE_BUT_THIS_API_IS_INTENDED_FOR_A_CLASS_DECLARATION_SHAPE_SUCH_AS_0_EFFECT_SERVICE_NOT_AS_CLASS;

// go: internal/rules/service_not_as_class.go:17 ServiceNotAsClass
pub static SERVICE_NOT_AS_CLASS: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "serviceNotAsClass",
    group: "style",
    description: "Warns when Context.Service is used as a variable instead of a class declaration",
    default_severity: Severity::Off,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_service_not_as_class(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[&m.suggested_usage],
            ));
        }
        diags
    },
});

// go: internal/rules/service_not_as_class.go:35 ServiceNotAsClassMatch
// ServiceNotAsClassMatch holds the data needed by both the diagnostic and the quickfix.
#[derive(Clone)]
pub struct ServiceNotAsClassMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,          // Error range for the call expression
    pub suggested_usage: String, // The full suggested class declaration string for the diagnostic message
    pub call_expr_node: P<Node>, // The call expression node (ServiceMap.Service<...>(...) or Context.Service<...>(...))
    pub variable_name: String,   // The variable/class name
    pub type_args_text: String,  // Text of original type arguments (e.g. "ConfigService")
    pub args_text: String,       // Text of original call arguments (e.g. `"Config"`)
    pub service_module: &'static str, // Service namespace identifier (e.g. "ServiceMap" or "Context")
    pub target_node: P<Node>, // The node to replace (variable statement or declaration list)
    pub modifier_nodes: Option<P<ModifierList>>, // Modifiers from the variable statement (e.g. export)
}

// go: internal/rules/service_not_as_class.go:50 AnalyzeServiceNotAsClass
// AnalyzeServiceNotAsClass finds all const variable declarations using the v4 service constructor
// that should be class declarations instead. V4-only rule.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`); Go ignores it, so it is dropped.
pub fn analyze_service_not_as_class(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<ServiceNotAsClassMatch> {
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }

    let mut matches = Vec::new();

    let mut node_to_visit: Vec<P<Node>> = Vec::new();
    sf.as_node().for_each_child(&mut |child| {
        node_to_visit.push(child);
        false
    });

    while let Some(node) = node_to_visit.pop() {
        if node.kind() == Kind::VariableDeclaration
            && let Some(m) = check_service_not_as_class(tp, sf, node)
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

// go: internal/rules/service_not_as_class.go:80 checkServiceNotAsClass
pub(crate) fn check_service_not_as_class(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Option<ServiceNotAsClassMatch> {
    let var_decl = node.as_variable_declaration();
    let initializer = var_decl.initializer()?;

    if initializer.kind() != Kind::CallExpression {
        return None;
    }

    let call_expr = initializer.as_call_expression();
    let type_args = call_expr.type_arguments().map_or(&[][..], |list| list.nodes());
    if type_args.is_empty() {
        return None;
    }

    // Check parent is a const declaration list
    let decl_list = node.parent()?;
    if decl_list.kind() != Kind::VariableDeclarationList {
        return None;
    }
    if !decl_list.flags().intersects(NodeFlags::Const) {
        return None;
    }

    let mut service_module = "";
    if tp.is_node_reference_to_effect_context_module_api(call_expr.expression(), "Service") {
        service_module = "Context";
    }
    if service_module.is_empty() {
        return None;
    }

    let text = sf.text();

    // Extract variable name
    let variable_name = extract_node_text(sf, text, node.name());

    // Extract type arguments text
    let type_arg_texts: Vec<String> =
        type_args.iter().map(|&ta| extract_node_text(sf, text, Some(ta))).collect();
    let type_args_text = type_arg_texts.join(", ");

    // Extract call arguments text
    let mut args_text = String::new();
    let arguments = call_expr.arguments().nodes();
    if !arguments.is_empty() {
        let arg_texts: Vec<String> =
            arguments.iter().map(|&arg| extract_node_text(sf, text, Some(arg))).collect();
        args_text = arg_texts.join(", ");
    }

    // Build suggested usage string using the matched service namespace.
    let suggested_usage = if !args_text.is_empty() {
        format!(
            "class {variable_name} extends {service_module}.Service<{variable_name}, {type_args_text}>()({args_text}) {{}}"
        )
    } else {
        format!(
            "class {variable_name} extends {service_module}.Service<{variable_name}, {type_args_text}>() {{}}"
        )
    };

    // Determine target node and modifiers
    let variable_statement = decl_list.parent();
    let target_node;
    let mut modifier_nodes = None;
    if let Some(variable_statement) = variable_statement
        && variable_statement.kind() == Kind::VariableStatement
    {
        target_node = variable_statement;
        modifier_nodes = variable_statement.as_variable_statement().modifiers();
    } else {
        target_node = decl_list;
    }

    Some(ServiceNotAsClassMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, initializer),
        suggested_usage,
        call_expr_node: initializer,
        variable_name,
        type_args_text,
        args_text,
        service_module,
        target_node,
        modifier_nodes,
    })
}

// go: internal/rules/service_not_as_class.go:169 extractNodeText
// extractNodeText gets the source text of a node, skipping leading trivia.
pub(crate) fn extract_node_text(sf: P<SourceFile>, text: &str, node: Option<P<Node>>) -> String {
    let Some(node) = node else {
        return String::new();
    };
    let start = i64::from(ets_shim::scanner::get_token_pos_of_node(node, sf, false));
    let end = node.end().source_offset().map_or(-1, i64::from);
    if start >= 0 && end >= start && end <= text.len() as i64 {
        return text[start as usize..end as usize].to_string();
    }
    String::new()
}
