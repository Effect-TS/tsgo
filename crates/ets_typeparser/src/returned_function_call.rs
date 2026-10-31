use ets_shim::ast::{Kind, Node};
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/returned_function_call.go:13 TypeParser.ReturnedFunctionCallOwner 56bfd066da599a7e
    // ReturnedFunctionCallOwner returns the name of the variable whose declared
    // type returns the function that call invokes, as for run(work) after
    // const run = Effect.runPromiseWith(context). It follows the resolved
    // signature's declaration out of the enclosing function types, so it returns
    // nil when the called function is not declared as another function's return
    // type.
    pub fn returned_function_call_owner(&mut self, call: P<Node>) -> Option<P<Node>> {
        if call.kind() != Kind::CallExpression {
            return None;
        }
        // Go: GetResolvedSignature never returns nil.
        let resolved = self.checker.get_resolved_signature_exported(call);
        let declaration = raw_signature(Some(resolved))?.declaration()?;
        let parent = declaration.parent()?;
        if (parent.kind() != Kind::FunctionType && parent.kind() != Kind::CallSignature)
            || parent.type_node() != Some(declaration)
        {
            return None;
        }
        let mut current = Some(parent);
        while let Some(node) = current {
            match node.kind() {
                Kind::FunctionType
                | Kind::CallSignature
                | Kind::TypeLiteral
                | Kind::ParenthesizedType => {
                    current = node.parent();
                    continue;
                }
                Kind::VariableDeclaration => return node.name(),
                _ => {}
            }
            return None;
        }
        None
    }
}
