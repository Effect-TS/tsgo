package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// extractCovariantType gets the type argument from a covariant property.
// Covariant<A> is encoded as () => A, so we get the return type.
func (tp *TypeParser) extractCovariantType(t *checker.Type, _ *ast.Node, propName string) *checker.Type {
	c := tp.checker
	propType := tp.GetTypeOfPropertyByName(t, propName)
	if propType == nil {
		return nil
	}
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
func (tp *TypeParser) extractContravariantType(t *checker.Type, _ *ast.Node, propName string) *checker.Type {
	c := tp.checker
	propType := tp.GetTypeOfPropertyByName(t, propName)
	if propType == nil {
		return nil
	}
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
func (tp *TypeParser) extractInvariantType(t *checker.Type, atLocation *ast.Node, propName string) *checker.Type {
	return tp.extractCovariantType(t, atLocation, propName)
}

// GetTypeOfPropertyByName returns the type of a property by name.
// Prefer this when only the property type is needed.
func (tp *TypeParser) GetTypeOfPropertyByName(t *checker.Type, name string) *checker.Type {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}
	return tp.checker.GetTypeOfPropertyOfType(t, name)
}

func (tp *TypeParser) resolveAliasedSymbol(sym *ast.Symbol) *ast.Symbol {
	if tp == nil || tp.checker == nil {
		return sym
	}
	c := tp.checker
	for sym != nil && sym.Flags&ast.SymbolFlagsAlias != 0 {
		sym = c.GetAliasedSymbol(sym)
	}
	return sym
}

// ResolveToGlobalSymbol follows aliases and up to two simple variable indirections
// so rules can recognize references to the original global symbol.
func (tp *TypeParser) ResolveToGlobalSymbol(sym *ast.Symbol) *ast.Symbol {
	if tp == nil || tp.checker == nil || sym == nil {
		return nil
	}
	c := tp.checker

	sym = tp.resolveAliasedSymbol(sym)
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
		next = tp.resolveAliasedSymbol(next)
		if next == sym {
			break
		}

		sym = next
		depth++
	}

	return sym
}
