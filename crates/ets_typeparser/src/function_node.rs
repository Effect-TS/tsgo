use ets_shim::ast::{Kind, Node, NodeList};
use ets_shim::core::P;

// go: internal/typeparser/function_node.go:10 GetFunctionLikeName
// GetFunctionLikeName returns the name string from a function-like node.
// Handles FunctionDeclaration, FunctionExpression, MethodDeclaration (with identifier name),
// and falls back to checking the parent VariableDeclaration for arrow/expression functions.
pub fn get_function_like_name(node: P<Node>) -> String {
    match node.kind() {
        Kind::FunctionDeclaration => {
            if let Some(name) = node.as_function_declaration().name() {
                return name.as_identifier().text().to_string();
            }
        }
        Kind::FunctionExpression => {
            if let Some(name) = node.as_function_expression().name() {
                return name.as_identifier().text().to_string();
            }
        }
        Kind::MethodDeclaration => {
            let name = node.as_method_declaration().name();
            if name.kind() == Kind::Identifier {
                return name.as_identifier().text().to_string();
            }
        }
        _ => {}
    }

    // Check parent variable declaration for arrow/expression functions
    if let Some(parent) = node.parent()
        && parent.kind() == Kind::VariableDeclaration
    {
        let name = parent.as_variable_declaration().name();
        if name.kind() == Kind::Identifier {
            return name.as_identifier().text().to_string();
        }
    }

    String::new()
}

// go: internal/typeparser/function_node.go:42 GetFunctionLikeBody
// GetFunctionLikeBody returns the body node from a function-like node.
// Handles FunctionDeclaration, FunctionExpression, ArrowFunction, and MethodDeclaration.
pub fn get_function_like_body(node: P<Node>) -> Option<P<Node>> {
    match node.kind() {
        Kind::FunctionDeclaration => node.as_function_declaration().body(),
        Kind::FunctionExpression => node.as_function_expression().body(),
        Kind::ArrowFunction => node.as_arrow_function().body(),
        Kind::MethodDeclaration => node.as_method_declaration().body(),
        _ => None,
    }
}

// go: internal/typeparser/function_node.go:70 GetFunctionLikeTypeParameters
// GetFunctionLikeTypeParameters returns the type parameters NodeList from a function-like node.
// Handles ArrowFunction, FunctionExpression, FunctionDeclaration, and MethodDeclaration.
pub fn get_function_like_type_parameters(node: P<Node>) -> Option<P<NodeList>> {
    match node.kind() {
        Kind::ArrowFunction => node.as_arrow_function().type_parameters(),
        Kind::FunctionExpression => node.as_function_expression().type_parameters(),
        Kind::FunctionDeclaration => node.as_function_declaration().type_parameters(),
        Kind::MethodDeclaration => node.as_method_declaration().type_parameters(),
        _ => None,
    }
}

// go: internal/typeparser/function_node.go:86 GetFunctionLikeParameters
// GetFunctionLikeParameters returns the parameters NodeList from a function-like node.
// Handles ArrowFunction, FunctionExpression, FunctionDeclaration, and MethodDeclaration.
pub fn get_function_like_parameters(node: P<Node>) -> Option<P<NodeList>> {
    match node.kind() {
        Kind::ArrowFunction => node.as_arrow_function().parameters(),
        Kind::FunctionExpression => node.as_function_expression().parameters(),
        Kind::FunctionDeclaration => node.as_function_declaration().parameters(),
        Kind::MethodDeclaration => node.as_method_declaration().parameters(),
        _ => None,
    }
}
