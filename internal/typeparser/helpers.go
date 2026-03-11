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
		if prop == nil || len(prop.Declarations) == 0 {
			continue
		}
		for _, decl := range prop.Declarations {
			if decl == nil {
				continue
			}
			nameNode := decl.Name()
			if nameNode == nil || nameNode.Kind != ast.KindComputedPropertyName {
				continue
			}
			computed := nameNode.AsComputedPropertyName()
			if computed == nil || computed.Expression == nil {
				continue
			}
			exprType := c.GetTypeAtLocation(computed.Expression)
			if exprType == nil {
				continue
			}
			if exprType.IsStringLiteral() {
				if lit, ok := exprType.AsLiteralType().Value().(string); ok && lit == name {
					return prop
				}
			}
			if sym := c.GetSymbolAtLocation(computed.Expression); sym != nil {
				if decl := sym.ValueDeclaration; decl != nil {
					if typeNode := decl.Type(); typeNode != nil {
						if tnode := c.GetTypeFromTypeNode(typeNode); tnode != nil && tnode.IsStringLiteral() {
							if lit, ok := tnode.AsLiteralType().Value().(string); ok && lit == name {
								return prop
							}
						}
					}
				}
			}
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
	for sym != nil && sym.Flags&ast.SymbolFlagsAlias != 0 {
		sym = c.GetAliasedSymbol(sym)
	}
	return sym
}

func symbolsMatch(c *checker.Checker, a *ast.Symbol, b *ast.Symbol) bool {
	if a == nil || b == nil {
		return false
	}
	if a == b {
		return true
	}
	if c != nil {
		if ea := c.GetExportSymbolOfSymbol(a); ea != nil {
			if eb := c.GetExportSymbolOfSymbol(b); eb != nil && ea == eb {
				return true
			}
		}
	}
	ma := c.GetMergedSymbol(a)
	mb := c.GetMergedSymbol(b)
	if ma == mb {
		return true
	}
	if len(a.Declarations) == 0 || len(b.Declarations) == 0 {
		return false
	}
	decls := make(map[*ast.Node]struct{}, len(a.Declarations))
	for _, d := range a.Declarations {
		if d != nil {
			decls[d] = struct{}{}
		}
	}
	for _, d := range b.Declarations {
		if d != nil {
			if _, ok := decls[d]; ok {
				return true
			}
		}
	}
	return false
}
