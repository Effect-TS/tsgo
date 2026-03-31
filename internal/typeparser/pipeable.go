package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// IsPipeableType returns true if the type has a callable "pipe" property,
// indicating it supports the pipeable pattern (e.g., value.pipe(f1, f2, ...)).
func IsPipeableType(c *checker.Checker, t *checker.Type, atLocation *ast.Node) bool {
	if c == nil || t == nil {
		return false
	}
	links := (&TypeParser{program: c.Program(), checker: c}).GetEffectLinks()
	return Cached(&links.IsPipeableType, t, func() bool {
		pipeSymbol := c.GetPropertyOfType(t, "pipe")
		if pipeSymbol == nil {
			return false
		}

		pipeType := c.GetTypeOfSymbolAtLocation(pipeSymbol, atLocation)
		signatures := c.GetSignaturesOfType(pipeType, checker.SignatureKindCall)
		return len(signatures) > 0
	})
}

// IsSafelyPipeableCallee returns true if a callee expression can be safely
// extracted into a pipe argument without losing `this` context.
// This is used by the missedPipeableOpportunity rule to determine which
// call expressions can be converted to pipe style.
func (tp *TypeParser) IsSafelyPipeableCallee(callee *ast.Node) bool {
	c := tp.checker
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
		sym := c.GetSymbolAtLocation(callee)
		if sym == nil {
			return false
		}

		// Module/namespace imports are safe
		if sym.Flags&(ast.SymbolFlagsModule|ast.SymbolFlagsNamespace|ast.SymbolFlagsValueModule) != 0 {
			return true
		}

		// Check if the symbol's declaration is a function, variable, or import (not a method)
		if len(sym.Declarations) > 0 {
			decl := sym.Declarations[0]
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
		sym := c.GetSymbolAtLocation(subject)
		if sym == nil {
			return false
		}

		// Check if subject is a module/namespace
		if sym.Flags&(ast.SymbolFlagsModule|ast.SymbolFlagsNamespace|ast.SymbolFlagsValueModule) != 0 {
			return true
		}

		// Check if the symbol's declaration indicates it's a module import
		if len(sym.Declarations) > 0 {
			decl := sym.Declarations[0]
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
