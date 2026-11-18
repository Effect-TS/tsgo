use ets_shim::ast::{Kind, Node};
use ets_shim::core::P;

// go: internal/typeparser/plain_parameter.go:9 IsPlainIdentifierParameter 1f9c0557b48b2b2f
// IsPlainIdentifierParameter reports whether parameter is a parameter
// declaration that binds a single identifier, without a rest marker or a
// default value. Optional markers, type annotations and `this` parameters (whose
// name is an identifier) are accepted: callers that care check them.
//
// Rust: a parameter declaration's name is never nil (`P<Node>`), so Go's nil check on it drops.
pub fn is_plain_identifier_parameter(parameter: impl Into<Option<P<Node>>>) -> bool {
    let Some(parameter) = parameter.into() else {
        return false;
    };
    if parameter.kind() != Kind::Parameter {
        return false;
    }
    let declaration = parameter.as_parameter_declaration();
    let name = declaration.name();
    name.kind() == Kind::Identifier
        && declaration.dot_dot_dot_token().is_none()
        && declaration.initializer().is_none()
}
