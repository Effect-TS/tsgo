use ets_shim::ast::{Kind, Node};
use ets_shim::checker::{ContextFlags, Type};
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/get_contextual_type.go:8 TypeParser.GetContextualType
    pub fn get_contextual_type(&mut self, node: P<Node>, flags: ContextFlags) -> Option<P<Type>> {
        if should_skip_inferred_generator_contextual_type(Some(node), flags) {
            return None;
        }
        self.checker.get_contextual_type(node, flags)
    }
}

// go: internal/typeparser/get_contextual_type.go:15 shouldSkipInferredGeneratorContextualType
pub(crate) fn should_skip_inferred_generator_contextual_type(
    node: Option<P<Node>>,
    flags: ContextFlags,
) -> bool {
    if !flags.intersects(ContextFlags::IgnoreNodeInferences) {
        return false;
    }
    let mut current = node;
    while let Some(n) = current {
        if n.kind() == Kind::FunctionExpression
            && n.as_function_expression().asterisk_token().is_some()
            && n.type_node().is_none()
        {
            return true;
        }
        current = n.parent();
    }
    false
}
