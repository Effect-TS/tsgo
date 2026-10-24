use ets_shim::ast::{Kind, Node};
use ets_shim::core::P;

// go: internal/typeparser/type_equivalent_node.go:15 TypeEquivalentNode
// TypeEquivalentNode returns the node whose type the checker computes when
// asked for the type of node, and true when that is a different node. Rules
// that type-check every node can skip node because the walk also visits the
// returned one:
//   - the name of a property access stands for the access (`b` in `a.b`);
//   - the name of a variable, parameter, property or object literal member
//     stands for its declaration;
//   - parentheses and `satisfies` stand for the wrapped expression.
pub fn type_equivalent_node(node: P<Node>) -> (P<Node>, bool) {
    let Some(parent) = node.parent() else {
        return (node, false);
    };
    match node.kind() {
        Kind::Identifier => {
            if parent.name() != Some(node) {
                return (node, false);
            }
            match parent.kind() {
                Kind::PropertyAccessExpression => {
                    if !ets_shim::ast::is_part_of_type_node(parent) {
                        return (parent, true);
                    }
                }
                Kind::VariableDeclaration
                | Kind::Parameter
                | Kind::PropertyDeclaration
                | Kind::PropertyAssignment
                | Kind::ShorthandPropertyAssignment => return (parent, true),
                _ => {}
            }
        }
        Kind::ParenthesizedExpression | Kind::SatisfiesExpression => {
            return (node.expression().expect("a wrapped expression"), true);
        }
        _ => {}
    }
    (node, false)
}
