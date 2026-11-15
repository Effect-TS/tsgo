package typeparser

import (
	"github.com/effect-ts/tsgo/internal/readonly"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

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
func (tp *TypeParser) ReferencesToSymbol(root *ast.Node, symbol *ast.Symbol) readonly.Slice[*ast.Node] {
	if tp == nil || tp.checker == nil || root == nil || symbol == nil {
		return readonly.Slice[*ast.Node]{}
	}
	var references []*ast.Node
	var visit ast.Visitor
	visit = func(node *ast.Node) bool {
		switch node.Kind {
		case ast.KindShorthandPropertyAssignment:
			if sameSymbolReference(tp.checker, tp.checker.GetShorthandAssignmentValueSymbol(node), symbol) {
				references = append(references, node.Name())
			}
		case ast.KindIdentifier:
			// A shorthand property's name was checked by its parent through the value symbol.
			if parent := node.Parent; parent == nil || parent.Kind != ast.KindShorthandPropertyAssignment || parent.Name() != node {
				if sameSymbolReference(tp.checker, tp.GetSymbolAtLocation(node), symbol) {
					references = append(references, node)
				}
			}
		}
		//ets:ast-walk scans a subtree for references to a symbol, which are identifiers, not flow steps
		node.ForEachChild(visit)
		return false
	}
	visit(root)
	return readonly.New(references)
}
