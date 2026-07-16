package typeparser

import "github.com/microsoft/typescript-go/shim/ast"

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

		declarationContainer := ast.FindAncestor(declarationNode, ast.IsFunctionOrSourceFile)
		locationContainer := ast.FindAncestor(location, ast.IsFunctionOrSourceFile)
		declarationStatement := ast.FindAncestor(declarationNode, ast.IsStatement)
		locationStatement := ast.FindAncestor(location, ast.IsStatement)
		return declarationContainer != nil && declarationContainer == locationContainer &&
			declarationStatement != nil && locationStatement != nil && declarationStatement.Parent == locationStatement.Parent &&
			declaration.Initializer.End() <= location.Pos()
	}

	return false
}
