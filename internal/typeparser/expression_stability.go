package typeparser

import "github.com/microsoft/TypeScript/tsc/shim/ast"

// IsExpressionValueStableAtLocation reports whether evaluating expression at
// location produces the same value as evaluating it at its original site.
func (tp *TypeParser) IsExpressionValueStableAtLocation(expression *ast.Node, location *ast.Node) bool {
	if tp == nil || tp.checker == nil || expression == nil || location == nil {
		return false
	}

	expression = ast.SkipParentheses(expression)
	if expression == nil {
		return false
	}

	switch expression.Kind {
	case ast.KindStringLiteral,
		ast.KindNumericLiteral,
		ast.KindBigIntLiteral,
		ast.KindNoSubstitutionTemplateLiteral,
		ast.KindTrueKeyword,
		ast.KindFalseKeyword,
		ast.KindNullKeyword:
		return true
	case ast.KindIdentifier:
		symbol := tp.GetSymbolAtLocation(expression)
		if expression.Text() == "undefined" {
			return symbol != nil && symbol == tp.checker.GetGlobalSymbol("undefined", ast.SymbolFlagsValue, nil)
		}
		if symbol == nil || symbol.ValueDeclaration == nil || symbol.ValueDeclaration.Kind != ast.KindVariableDeclaration {
			return false
		}

		declarationNode := symbol.ValueDeclaration
		declaration := declarationNode.AsVariableDeclaration()
		if declaration == nil || declaration.Initializer == nil || declarationNode.Parent == nil || declarationNode.Parent.Kind != ast.KindVariableDeclarationList {
			return false
		}
		if declarationNode.Parent.Flags&ast.NodeFlagsConst == 0 || ast.GetSourceFileOfNode(declarationNode) != ast.GetSourceFileOfNode(location) {
			return false
		}

		// Symbol resolution already proves the declaration is visible here. A
		// const's value is stable across nested lexical scopes as long as its
		// initializer occurs before the use; the same-container restriction would
		// incorrectly reject module constants referenced inside a function.
		return declaration.Initializer.End() <= location.Pos()
	}

	return false
}
