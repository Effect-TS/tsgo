package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// extractCovariantType gets the type argument from a covariant property.
// Covariant<A> is encoded as () => A, so we get the return type.
func extractCovariantType(c *checker.Checker, t *checker.Type, atLocation *ast.Node, propName string) *checker.Type {
	propSymbol := c.GetPropertyOfType(t, propName)
	if propSymbol == nil {
		return nil
	}

	propType := c.GetTypeOfSymbolAtLocation(propSymbol, atLocation)
	signatures := c.GetSignaturesOfType(propType, checker.SignatureKindCall)

	if len(signatures) != 1 {
		return nil
	}

	if len(signatures[0].TypeParameters()) > 0 {
		return nil
	}

	return c.GetReturnTypeOfSignature(signatures[0])
}

// extractContravariantType gets the type argument from a contravariant property.
// Contravariant<A> is encoded as (_: A) => void, so we get the first parameter type.
func extractContravariantType(c *checker.Checker, t *checker.Type, atLocation *ast.Node, propName string) *checker.Type {
	propSymbol := c.GetPropertyOfType(t, propName)
	if propSymbol == nil {
		return nil
	}

	propType := c.GetTypeOfSymbolAtLocation(propSymbol, atLocation)
	signatures := c.GetSignaturesOfType(propType, checker.SignatureKindCall)

	if len(signatures) != 1 {
		return nil
	}

	if len(signatures[0].TypeParameters()) > 0 {
		return nil
	}

	params := signatures[0].Parameters()
	if len(params) == 0 {
		return nil
	}

	return c.GetTypeOfSymbol(params[0])
}

// extractInvariantType gets the type argument from an invariant property.
// Invariant<A> is encoded as (_: A) => A, so we extract the return type (same as covariant).
func extractInvariantType(c *checker.Checker, t *checker.Type, atLocation *ast.Node, propName string) *checker.Type {
	return extractCovariantType(c, t, atLocation, propName)
}

// GetPropertyOfTypeByName returns a property symbol by name, including computed properties backed by string literals.
func GetPropertyOfTypeByName(c *checker.Checker, t *checker.Type, name string) *ast.Symbol {
	if c == nil || t == nil {
		return nil
	}
	if sym := c.GetPropertyOfType(t, name); sym != nil {
		return sym
	}
	for _, prop := range c.GetPropertiesOfType(t) {
		if prop == nil {
			continue
		}
		nameType := checker.Checker_getLiteralTypeFromProperty(c, prop, checker.TypeFlagsStringOrNumberLiteralOrUnique, true)
		if nameType == nil || !nameType.IsStringLiteral() {
			continue
		}
		if lit, ok := nameType.AsLiteralType().Value().(string); ok && lit == name {
			return prop
		}
	}
	return nil
}

func moduleSymbolFromSourceFile(c *checker.Checker, sf *ast.SourceFile) *ast.Symbol {
	if c == nil || sf == nil {
		return nil
	}
	sym := sf.AsNode().Symbol()
	if sym == nil {
		return nil
	}
	return c.GetMergedSymbol(sym)
}

func resolveAliasedSymbol(c *checker.Checker, sym *ast.Symbol) *ast.Symbol {
	if c == nil {
		return sym
	}
	for sym != nil && sym.Flags&ast.SymbolFlagsAlias != 0 {
		sym = c.GetAliasedSymbol(sym)
	}
	return sym
}

// ResolveToGlobalSymbol follows aliases and up to two simple variable indirections
// so rules can recognize references to the original global symbol.
func ResolveToGlobalSymbol(c *checker.Checker, sym *ast.Symbol) *ast.Symbol {
	if c == nil || sym == nil {
		return nil
	}

	sym = resolveAliasedSymbol(c, sym)
	depth := 0
	for depth < 2 && sym != nil && sym.ValueDeclaration != nil && sym.ValueDeclaration.Kind == ast.KindVariableDeclaration {
		decl := sym.ValueDeclaration.AsVariableDeclaration()
		if decl == nil || decl.Initializer == nil {
			break
		}

		next := c.GetSymbolAtLocation(decl.Initializer)
		if next == nil {
			break
		}
		next = resolveAliasedSymbol(c, next)
		if next == sym {
			break
		}

		sym = next
		depth++
	}

	return sym
}
