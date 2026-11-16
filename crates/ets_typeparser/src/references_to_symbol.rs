use ets_shim::ast::{Kind, Node, Symbol};
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/references_to_symbol.go:20 TypeParser.ReferencesToSymbol 5dd9ce3df63bf25a
    // ReferencesToSymbol returns, in source order, the nodes in root's subtree
    // (root included) that refer to symbol. A node refers to symbol when it is:
    //   - an Identifier whose symbol (GetSymbolAtLocation) is the same reference as
    //     symbol per checker.Checker_getSymbolIfSameReference, so import aliases and
    //     merged or exported local symbols match;
    //   - the name of a shorthand property assignment `{ x }` whose value symbol
    //     (GetShorthandAssignmentValueSymbol) is the same reference as symbol.
    //
    // Every returned node is an Identifier: for a shorthand property the name is
    // returned, once, so callers can treat `{ x }` like any other use of `x`. A
    // binding with the same name but another symbol (a shadowing declaration and its
    // uses) is not a reference. A nil root or symbol returns an empty slice.
    //
    // Rust: Go's `readonly.Slice` of a fresh slice is an owned `Vec`.
    pub fn references_to_symbol(
        &mut self,
        root: impl Into<Option<P<Node>>>,
        symbol: impl Into<Option<P<Symbol>>>,
    ) -> Vec<P<Node>> {
        let (Some(root), Some(symbol)) = (root.into(), symbol.into()) else {
            return Vec::new();
        };
        let mut references = Vec::new();

        // Rust: Go's recursive `visit` closure is a nested function.
        fn visit(
            tp: &mut TypeParser<'_>,
            node: P<Node>,
            symbol: P<Symbol>,
            references: &mut Vec<P<Node>>,
        ) -> bool {
            match node.kind() {
                Kind::ShorthandPropertyAssignment => {
                    let value_symbol = tp.checker.get_shorthand_assignment_value_symbol(Some(node));
                    if same_symbol_reference(tp.checker, value_symbol, Some(symbol)) {
                        references.push(node.name().expect("a shorthand property has a name"));
                    }
                }
                Kind::Identifier => {
                    // A shorthand property's name was checked by its parent through the value symbol.
                    let shorthand_name = node.parent().is_some_and(|parent| {
                        parent.kind() == Kind::ShorthandPropertyAssignment
                            && parent.name() == Some(node)
                    });
                    if !shorthand_name {
                        let node_symbol = tp.get_symbol_at_location(node);
                        if same_symbol_reference(tp.checker, node_symbol, Some(symbol)) {
                            references.push(node);
                        }
                    }
                }
                _ => {}
            }
            node.for_each_child(&mut |child| visit(tp, child, symbol, references));
            false
        }
        visit(self, root, symbol, &mut references);
        references
    }
}
