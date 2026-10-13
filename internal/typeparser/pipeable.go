package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// IsPipeableType returns true if the type has a callable "pipe" property,
// indicating it supports the pipeable pattern (e.g., value.pipe(f1, f2, ...)).
func (tp *TypeParser) IsPipeableType(t *checker.Type) bool {
	if tp == nil || tp.checker == nil || t == nil {
		return false
	}
	c := tp.checker
	return Cached(&tp.links.IsPipeableType, t, func() bool {
		pipeType := tp.GetTypeOfPropertyByName(t, "pipe")
		if pipeType == nil {
			return false
		}
		signatures := c.GetSignaturesOfType(pipeType, checker.SignatureKindCall)
		return len(signatures) > 0
	})
}

// LiteralCannotBePipeable reports, without computing the literal's type, that an
// object or array literal has no `pipe` property. An object literal's type only
// has the properties it declares plus those of the global Object interface; an
// array literal's type is an array or tuple whose members come from Array or
// ReadonlyArray and Object. Spreads and computed names could add `pipe`, and
// the global interfaces are read through their declared types so members
// inherited from augmented base interfaces count.
func (tp *TypeParser) LiteralCannotBePipeable(node *ast.Node) bool {
	if tp == nil || tp.checker == nil || node == nil {
		return false
	}
	node = ast.SkipParentheses(node)
	switch node.Kind {
	case ast.KindObjectLiteralExpression:
		for _, member := range node.AsObjectLiteralExpression().Properties.Nodes {
			name := member.Name()
			if member.Kind == ast.KindSpreadAssignment || name == nil || name.Kind == ast.KindComputedPropertyName || ast.GetTextOfPropertyName(name) == "pipe" {
				return false
			}
		}
		return !tp.globalInterfaceHasPipe("Object")
	case ast.KindArrayLiteralExpression:
		return !tp.globalInterfaceHasPipe("Object") && !tp.globalInterfaceHasPipe("Array") && !tp.globalInterfaceHasPipe("ReadonlyArray")
	}
	return false
}

func (tp *TypeParser) globalInterfaceHasPipe(name string) bool {
	return Cached(&tp.links.GlobalInterfaceHasPipe, name, func() bool {
		symbol := tp.checker.GetGlobalSymbol(name, ast.SymbolFlagsType, nil)
		if symbol == nil {
			return false
		}
		declared := tp.checker.GetDeclaredTypeOfSymbol(symbol)
		return declared != nil && tp.checker.GetPropertyOfType(declared, "pipe") != nil
	})
}

// IsSafelyPipeableCallee returns true if a callee expression can be safely
// extracted into a pipe argument without losing `this` context.
// This is used by the missedPipeableOpportunity rule to determine which
// call expressions can be converted to pipe style.
func (tp *TypeParser) IsSafelyPipeableCallee(callee *ast.Node) bool {
	if callee == nil {
		return false
	}

	// Call expressions are safe - they return a value
	if ast.IsCallExpression(callee) {
		return true
	}

	// Arrow functions are safe - no `this` binding
	if callee.Kind == ast.KindArrowFunction {
		return true
	}

	// Function expressions are safe
	if callee.Kind == ast.KindFunctionExpression {
		return true
	}

	// Parenthesized expressions - check inner
	if callee.Kind == ast.KindParenthesizedExpression {
		return tp.IsSafelyPipeableCallee(callee.AsParenthesizedExpression().Expression)
	}

	// Simple identifiers - check if it's a module/namespace or standalone function
	if ast.IsIdentifier(callee) {
		sym := tp.GetSymbolAtLocation(callee)
		if sym == nil {
			return false
		}

		// Module/namespace imports are safe
		if ast.TSGetSymbolFlags(sym)&(ast.SymbolFlagsModule|ast.SymbolFlagsNamespace|ast.SymbolFlagsValueModule) != 0 {
			return true
		}

		// Check if the symbol's declaration is a function, variable, or import (not a method)
		if len(ast.TSGetSymbolDeclarations(sym)) > 0 {
			decl := ast.TSGetSymbolDeclarations(sym)[0]
			switch decl.Kind {
			case ast.KindFunctionDeclaration,
				ast.KindVariableDeclaration,
				ast.KindImportSpecifier,
				ast.KindImportClause,
				ast.KindNamespaceImport:
				return true
			}
		}

		return false
	}

	// Property access - check if subject is a module/namespace
	if ast.IsPropertyAccessExpression(callee) {
		propAccess := callee.AsPropertyAccessExpression()
		subject := propAccess.Expression
		sym := tp.GetSymbolAtLocation(subject)
		if sym == nil {
			return false
		}

		// Check if subject is a module/namespace
		if ast.TSGetSymbolFlags(sym)&(ast.SymbolFlagsModule|ast.SymbolFlagsNamespace|ast.SymbolFlagsValueModule) != 0 {
			return true
		}

		// Check if the symbol's declaration indicates it's a module import
		if len(ast.TSGetSymbolDeclarations(sym)) > 0 {
			decl := ast.TSGetSymbolDeclarations(sym)[0]
			switch decl.Kind {
			case ast.KindNamespaceImport,
				ast.KindSourceFile,
				ast.KindModuleDeclaration:
				return true
			}
		}

		return false
	}

	return false
}
