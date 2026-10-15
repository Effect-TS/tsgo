package typeparser

import (
	"strings"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

// NodeCannotBeEffectLibraryType reports whether node is an expression whose
// type can never be an Effect library data type such as Effect, Stream or
// Layer. Literals, functions, and operators that always produce a primitive
// have no properties that could match. The `undefined` identifier is left out
// because a local binding can shadow it. Object literals are only ruled out in
// v4, where those types carry a "~effect/..." key; v3 detection is structural
// and could match a plain object literal.
func (tp *TypeParser) NodeCannotBeEffectLibraryType(node *ast.Node) bool {
	switch node.Kind {
	case ast.KindArrayLiteralExpression, ast.KindArrowFunction, ast.KindFunctionExpression,
		ast.KindNumericLiteral, ast.KindBigIntLiteral, ast.KindTemplateExpression, ast.KindNoSubstitutionTemplateLiteral,
		ast.KindTrueKeyword, ast.KindFalseKeyword, ast.KindNullKeyword, ast.KindRegularExpressionLiteral,
		ast.KindPrefixUnaryExpression, ast.KindVoidExpression, ast.KindTypeOfExpression:
		return true
	case ast.KindStringLiteral:
		// A string literal property name stands for the property's type.
		return node.Parent == nil || node.Parent.Name() != node
	case ast.KindObjectLiteralExpression:
		if tp == nil || tp.checker == nil || tp.DetectEffectVersion() != EffectMajorV4 {
			return false
		}
		for _, property := range node.AsObjectLiteralExpression().Properties.Nodes {
			name := property.Name()
			if name == nil || name.Kind == ast.KindComputedPropertyName || strings.HasPrefix(name.Text(), "~effect/") {
				return false
			}
		}
		return true
	}
	return false
}
