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

// IsExpressionEvaluableAtLocation reports whether evaluating expression once at
// location, instead of at its own deferred site, reads the same bindings. Reads
// inside nested function bodies still happen when those functions run, so only
// the eagerly evaluated part of expression is checked: it must not read a
// mutable binding or a binding that is still uninitialized at location.
func (tp *TypeParser) IsExpressionEvaluableAtLocation(expression *ast.Node, location *ast.Node) bool {
	if tp == nil || tp.checker == nil || expression == nil || location == nil {
		return false
	}

	evaluable := true
	var visit func(node *ast.Node) bool
	visit = func(node *ast.Node) bool {
		if node == nil || ast.IsPartOfTypeNode(node) {
			return false
		}
		if ast.IsFunctionLikeDeclaration(node) {
			// The body runs later; a computed name is evaluated with the enclosing expression.
			if name := node.Name(); name != nil && ast.IsComputedPropertyName(name) {
				visit(name)
			}
			return !evaluable
		}
		if node.Kind == ast.KindIdentifier {
			if symbol, ok := tp.evaluatedReferenceSymbol(node); ok &&
				!tp.isBindingEvaluableAtLocation(symbol, expression, location) {
				evaluable = false
			}
			return !evaluable
		}
		node.ForEachChild(visit)
		return !evaluable
	}
	visit(expression)
	return evaluable
}

// evaluatedReferenceSymbol resolves an identifier that reads a binding. The
// second result is false for identifiers that name a declaration, property or
// label instead of reading a value.
func (tp *TypeParser) evaluatedReferenceSymbol(node *ast.Node) (*ast.Symbol, bool) {
	parent := node.Parent
	if parent != nil && ast.IsShorthandPropertyAssignment(parent) && parent.Name() == node {
		return tp.checker.GetShorthandAssignmentValueSymbol(parent), true
	}
	if ast.IsDeclarationNameOrImportPropertyName(node) || ast.IsLabelName(node) {
		return nil, false
	}
	if parent != nil {
		switch parent.Kind {
		case ast.KindPropertyAccessExpression:
			if parent.Name() == node {
				return nil, false
			}
		case ast.KindQualifiedName:
			if parent.AsQualifiedName().Right == node {
				return nil, false
			}
		case ast.KindPropertyAssignment, ast.KindBindingElement:
			if parent.Name() == node || parent.PropertyName() == node {
				return nil, false
			}
		case ast.KindMetaProperty:
			return nil, false
		}
	}
	return tp.GetSymbolAtLocation(node), true
}

// isBindingEvaluableAtLocation reports whether reading symbol at location
// observes the value it holds when read from within expression.
func (tp *TypeParser) isBindingEvaluableAtLocation(symbol *ast.Symbol, expression *ast.Node, location *ast.Node) bool {
	if symbol == nil {
		return false
	}
	if ast.TSGetSymbolFlags(symbol)&ast.SymbolFlagsAlias != 0 {
		return true
	}

	sourceFile := ast.GetSourceFileOfNode(location)
	var declarations []*ast.Node
	for _, declaration := range ast.TSGetSymbolDeclarations(symbol) {
		if declaration != nil && ast.GetSourceFileOfNode(declaration) == sourceFile {
			declarations = append(declarations, declaration)
		}
	}
	// Globals from lib and other files are initialized before this module runs.
	if len(declarations) == 0 {
		return true
	}

	for _, declaration := range declarations {
		if ast.IsNodeDescendantOf(declaration, expression) {
			continue
		}
		root := ast.GetRootDeclaration(declaration)
		switch {
		case root.Kind == ast.KindParameter ||
			ast.IsVariableDeclaration(root) && root.Parent != nil && root.Parent.Kind == ast.KindCatchClause:
			if checker.Checker_isSymbolAssigned(tp.checker, symbol) {
				return false
			}
		case ast.IsVariableDeclaration(root):
			if !ast.IsVarConst(root) || !tp.isDeclaredBeforeLocation(root, location) {
				return false
			}
		case root.Kind == ast.KindFunctionDeclaration:
			// Function declarations are hoisted with their value.
		default:
			if !tp.isDeclaredBeforeLocation(root, location) {
				return false
			}
		}
	}
	return true
}
