package rules

import (
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// arrowIgnoresParameter reports whether handler is a synchronous, non-generic
// arrow that cannot observe the value it is called with. Restrict callbacks to
// arrows so the value cannot be observed through a function's arguments
// object. An unused plain parameter can be removed.
func arrowIgnoresParameter(tp *typeparser.TypeParser, c *checker.Checker, handler *ast.Node) bool {
	if handler == nil || handler.Kind != ast.KindArrowFunction || ast.GetCombinedModifierFlags(handler)&ast.ModifierFlagsAsync != 0 {
		return false
	}
	if params := typeparser.GetFunctionLikeTypeParameters(handler); params != nil && len(params.Nodes) != 0 {
		return false
	}
	params := typeparser.GetFunctionLikeParameters(handler)
	if params == nil || len(params.Nodes) == 0 {
		return true
	}
	if len(params.Nodes) != 1 {
		return false
	}
	parameter := params.Nodes[0].AsParameterDeclaration()
	if parameter == nil || parameter.Name().Kind != ast.KindIdentifier || parameter.Initializer != nil || parameter.DotDotDotToken != nil {
		return false
	}
	symbol := tp.GetSymbolAtLocation(parameter.Name())
	if symbol == nil {
		return false
	}
	var usesParameter func(*ast.Node) bool
	usesParameter = func(node *ast.Node) bool {
		if node.Kind == ast.KindShorthandPropertyAssignment && c.GetShorthandAssignmentValueSymbol(node) == symbol {
			return true
		}
		if node.Kind == ast.KindIdentifier && tp.GetSymbolAtLocation(node) == symbol {
			return true
		}
		return node.ForEachChild(usesParameter)
	}
	return !usesParameter(handler.AsArrowFunction().Body) && (handler.AsArrowFunction().Type == nil || !usesParameter(handler.AsArrowFunction().Type))
}
