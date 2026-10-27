package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

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
	case ast.KindPrefixUnaryExpression:
		// -1 and -1n are signed literals; no other unary forms are accepted.
		unary := expression.AsPrefixUnaryExpression()
		return unary.Operator == ast.KindMinusToken && unary.Operand != nil &&
			(unary.Operand.Kind == ast.KindNumericLiteral || unary.Operand.Kind == ast.KindBigIntLiteral)
	case ast.KindPropertyAccessExpression:
		// Enum members cannot be reassigned; the receiver must be the enum itself.
		access := expression.AsPropertyAccessExpression()
		if !ast.IsEntityNameExpression(access.Expression) {
			return false
		}
		enumSymbol := tp.GetSymbolAtLocation(access.Expression)
		member := tp.GetSymbolAtLocation(access.Name())
		if enumSymbol == nil || member == nil ||
			ast.TSGetSymbolFlags(enumSymbol)&ast.SymbolFlagsEnum == 0 ||
			ast.TSGetSymbolFlags(member)&ast.SymbolFlagsEnumMember == 0 {
			return false
		}
		declaration := ast.TSGetSymbolValueDeclaration(member)
		return declaration != nil && ast.IsEnumMember(declaration) &&
			tp.isDeclaredBeforeLocation(declaration.Parent, location)
	case ast.KindIdentifier:
		symbol := tp.GetSymbolAtLocation(expression)
		if expression.Text() == "undefined" {
			return symbol != nil && symbol == tp.checker.GetGlobalSymbol("undefined", ast.SymbolFlagsValue, nil)
		}
		if symbol == nil || !checker.Checker_isConstantVariable(tp.checker, symbol) {
			return false
		}
		declaration := ast.TSGetSymbolValueDeclaration(symbol)
		if declaration == nil || !ast.IsVariableDeclaration(declaration) {
			return false
		}
		// A `declare const` has no initializer, but its value cannot be reassigned.
		if declaration.Initializer() == nil && declaration.Flags&ast.NodeFlagsAmbient == 0 {
			return false
		}
		return tp.isDeclaredBeforeLocation(declaration, location)
	}

	return false
}

// isDeclaredBeforeLocation reports whether declaration's value already exists
// when location is evaluated. The checker alone accepts later declarations used
// from deferred function bodies, so the positional check is kept alongside it.
func (tp *TypeParser) isDeclaredBeforeLocation(declaration *ast.Node, location *ast.Node) bool {
	return declaration != nil &&
		ast.GetSourceFileOfNode(declaration) == ast.GetSourceFileOfNode(location) &&
		declaration.End() <= location.Pos() &&
		checker.Checker_isBlockScopedNameDeclaredBeforeUse(tp.checker, declaration, location)
}
