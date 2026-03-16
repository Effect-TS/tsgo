package typeparser

import (
	"sort"
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// parseSchemaVarianceStruct checks if a type is a Schema variance struct (has _A, _I, _R).
func parseSchemaVarianceStruct(c *checker.Checker, t *checker.Type, atLocation *ast.Node) bool {
	a := extractInvariantType(c, t, atLocation, "_A")
	if a == nil {
		return false
	}
	i := extractInvariantType(c, t, atLocation, "_I")
	if i == nil {
		return false
	}
	r := extractCovariantType(c, t, atLocation, "_R")
	return r != nil
}

// isSchemaType checks if a type is a Schema type (v4 or v3).
func isSchemaType(c *checker.Checker, t *checker.Type, atLocation *ast.Node) bool {
	if c == nil || t == nil {
		return false
	}
	links := GetEffectLinks(c)
	return Cached(&links.IsSchemaType, t, func() bool {
		version := DetectEffectVersion(c)
		if version == EffectMajorV4 {
			return GetPropertyOfTypeByName(c, t, SchemaTypeId) != nil
		}

		// v3 / unknown: check for 'ast' property first
		if c.GetPropertyOfType(t, "ast") == nil {
			return false
		}

		props := c.GetPropertiesOfType(t)
		var candidates []*ast.Symbol
		for _, prop := range props {
			if prop == nil {
				continue
			}
			if prop.Flags&ast.SymbolFlagsProperty == 0 {
				continue
			}
			if prop.Flags&ast.SymbolFlagsOptional != 0 {
				continue
			}
			if prop.ValueDeclaration == nil {
				continue
			}
			candidates = append(candidates, prop)
		}

		if len(candidates) == 0 {
			return false
		}

		// Sort so properties containing "TypeId" come first (optimization heuristic)
		sort.SliceStable(candidates, func(i, j int) bool {
			iHas := strings.Contains(candidates[i].Name, "TypeId")
			jHas := strings.Contains(candidates[j].Name, "TypeId")
			if iHas && !jHas {
				return true
			}
			return false
		})

		for _, prop := range candidates {
			propType := c.GetTypeOfSymbolAtLocation(prop, atLocation)
			if parseSchemaVarianceStruct(c, propType, atLocation) {
				return true
			}
		}

		return false
	})
}

// IsSchemaType returns true if the type is a Schema type (v4 or v3).
func IsSchemaType(c *checker.Checker, t *checker.Type, atLocation *ast.Node) bool {
	return isSchemaType(c, t, atLocation)
}

// SchemaTypes holds the A (Type) and E (Encoded) types extracted from a Schema type.
type SchemaTypes struct {
	A *checker.Type
	E *checker.Type
}

// EffectSchemaTypes extracts the A (Type) and E (Encoded) types from a Schema type.
// Returns nil if the type is not a recognized Schema type or types cannot be extracted.
func EffectSchemaTypes(c *checker.Checker, t *checker.Type, atLocation *ast.Node) *SchemaTypes {
	if c == nil || t == nil {
		return nil
	}
	links := GetEffectLinks(c)
	return Cached(&links.EffectSchemaTypes, t, func() *SchemaTypes {
		version := DetectEffectVersion(c)
		if version == EffectMajorV4 {
			if GetPropertyOfTypeByName(c, t, SchemaTypeId) == nil {
				return nil
			}
			// V4: get Type and Encoded properties directly
			aType := getPropertyType(c, t, atLocation, "Type")
			eType := getPropertyType(c, t, atLocation, "Encoded")
			if aType == nil || eType == nil {
				return nil
			}
			return &SchemaTypes{A: aType, E: eType}
		}

		// V3: check for 'ast' property first
		if c.GetPropertyOfType(t, "ast") == nil {
			return nil
		}

		// Find the variance struct property and extract A/I types
		props := c.GetPropertiesOfType(t)
		for _, prop := range props {
			if prop == nil || prop.Flags&ast.SymbolFlagsProperty == 0 || prop.Flags&ast.SymbolFlagsOptional != 0 || prop.ValueDeclaration == nil {
				continue
			}
			propType := c.GetTypeOfSymbolAtLocation(prop, atLocation)
			a := extractInvariantType(c, propType, atLocation, "_A")
			if a == nil {
				continue
			}
			i := extractInvariantType(c, propType, atLocation, "_I")
			if i == nil {
				continue
			}
			r := extractCovariantType(c, propType, atLocation, "_R")
			if r == nil {
				continue
			}
			return &SchemaTypes{A: a, E: i}
		}

		return nil
	})
}

// getPropertyType extracts the type of a named property from a type.
func getPropertyType(c *checker.Checker, t *checker.Type, atLocation *ast.Node, propName string) *checker.Type {
	sym := c.GetPropertyOfType(t, propName)
	if sym == nil {
		return nil
	}
	return c.GetTypeOfSymbolAtLocation(sym, atLocation)
}

func isSchemaTypeSourceFile(c *checker.Checker, sf *ast.SourceFile) bool {
	if c == nil || sf == nil {
		return false
	}

	moduleSym := moduleSymbolFromSourceFile(c, sf)
	if moduleSym == nil {
		return false
	}

	schemaSym := c.TryGetMemberInModuleExportsAndProperties("Schema", moduleSym)
	if schemaSym == nil {
		return false
	}

	schemaType := c.GetDeclaredTypeOfSymbol(schemaSym)
	if schemaType == nil {
		return false
	}

	return isSchemaType(c, schemaType, sf.AsNode())
}

// IsNodeReferenceToEffectSchemaModuleApi reports whether node resolves to a member
// exported by the "effect" package from a module that exports the Schema type.
func IsNodeReferenceToEffectSchemaModuleApi(c *checker.Checker, node *ast.Node, memberName string) bool {
	if c == nil || node == nil {
		return false
	}

	sym := c.GetSymbolAtLocation(node)
	if sym == nil && node.Kind == ast.KindPropertyAccessExpression {
		if prop := node.AsPropertyAccessExpression(); prop != nil && prop.Name() != nil {
			sym = c.GetSymbolAtLocation(prop.Name())
		}
	}
	sym = resolveAliasedSymbol(c, sym)
	if sym == nil {
		return false
	}

	for _, decl := range sym.Declarations {
		if decl == nil {
			continue
		}
		sf := ast.GetSourceFileOfNode(decl)
		if sf == nil {
			continue
		}
		pkg := PackageJsonForSourceFile(c, sf)
		if pkg == nil {
			continue
		}
		if name, ok := pkg.Name.GetValue(); ok && strings.EqualFold(name, "effect") {
			if !isSchemaTypeSourceFile(c, sf) {
				continue
			}
			moduleSym := moduleSymbolFromSourceFile(c, sf)
			if moduleSym == nil {
				continue
			}
			exportSym := c.TryGetMemberInModuleExportsAndProperties(memberName, moduleSym)
			if exportSym == nil {
				continue
			}
			exportSym = resolveAliasedSymbol(c, exportSym)
			if symbolsMatch(c, exportSym, sym) {
				return true
			}
		}
	}

	return false
}

func isParseResultSourceFile(c *checker.Checker, sf *ast.SourceFile) bool {
	if c == nil || sf == nil {
		return false
	}

	moduleSym := moduleSymbolFromSourceFile(c, sf)
	if moduleSym == nil {
		return false
	}

	// Check for ParseIssue type
	if c.TryGetMemberInModuleExportsAndProperties("ParseIssue", moduleSym) == nil {
		return false
	}

	// Check for decodeSync export
	if c.TryGetMemberInModuleExportsAndProperties("decodeSync", moduleSym) == nil {
		return false
	}

	// Check for encodeSync export
	if c.TryGetMemberInModuleExportsAndProperties("encodeSync", moduleSym) == nil {
		return false
	}

	return true
}

// IsNodeReferenceToEffectParseResultModuleApi reports whether node resolves to a member
// exported by the "effect" package from a module that exports the ParseResult type (V3).
func IsNodeReferenceToEffectParseResultModuleApi(c *checker.Checker, node *ast.Node, memberName string) bool {
	if c == nil || node == nil {
		return false
	}

	sym := c.GetSymbolAtLocation(node)
	if sym == nil && node.Kind == ast.KindPropertyAccessExpression {
		if prop := node.AsPropertyAccessExpression(); prop != nil && prop.Name() != nil {
			sym = c.GetSymbolAtLocation(prop.Name())
		}
	}
	sym = resolveAliasedSymbol(c, sym)
	if sym == nil {
		return false
	}

	for _, decl := range sym.Declarations {
		if decl == nil {
			continue
		}
		sf := ast.GetSourceFileOfNode(decl)
		if sf == nil {
			continue
		}
		pkg := PackageJsonForSourceFile(c, sf)
		if pkg == nil {
			continue
		}
		if name, ok := pkg.Name.GetValue(); ok && strings.EqualFold(name, "effect") {
			if !isParseResultSourceFile(c, sf) {
				continue
			}
			moduleSym := moduleSymbolFromSourceFile(c, sf)
			if moduleSym == nil {
				continue
			}
			exportSym := c.TryGetMemberInModuleExportsAndProperties(memberName, moduleSym)
			if exportSym == nil {
				continue
			}
			exportSym = resolveAliasedSymbol(c, exportSym)
			if symbolsMatch(c, exportSym, sym) {
				return true
			}
		}
	}

	return false
}

func isSchemaParserSourceFile(c *checker.Checker, sf *ast.SourceFile) bool {
	if c == nil || sf == nil {
		return false
	}

	moduleSym := moduleSymbolFromSourceFile(c, sf)
	if moduleSym == nil {
		return false
	}

	// Check for decodeEffect export
	if c.TryGetMemberInModuleExportsAndProperties("decodeEffect", moduleSym) == nil {
		return false
	}

	// Check for encodeEffect export
	if c.TryGetMemberInModuleExportsAndProperties("encodeEffect", moduleSym) == nil {
		return false
	}

	return true
}

// IsNodeReferenceToEffectSchemaParserModuleApi reports whether node resolves to a member
// exported by the "effect" package from a module that exports the SchemaParser type (V4).
func IsNodeReferenceToEffectSchemaParserModuleApi(c *checker.Checker, node *ast.Node, memberName string) bool {
	if c == nil || node == nil {
		return false
	}

	sym := c.GetSymbolAtLocation(node)
	if sym == nil && node.Kind == ast.KindPropertyAccessExpression {
		if prop := node.AsPropertyAccessExpression(); prop != nil && prop.Name() != nil {
			sym = c.GetSymbolAtLocation(prop.Name())
		}
	}
	sym = resolveAliasedSymbol(c, sym)
	if sym == nil {
		return false
	}

	for _, decl := range sym.Declarations {
		if decl == nil {
			continue
		}
		sf := ast.GetSourceFileOfNode(decl)
		if sf == nil {
			continue
		}
		pkg := PackageJsonForSourceFile(c, sf)
		if pkg == nil {
			continue
		}
		if name, ok := pkg.Name.GetValue(); ok && strings.EqualFold(name, "effect") {
			if !isSchemaParserSourceFile(c, sf) {
				continue
			}
			moduleSym := moduleSymbolFromSourceFile(c, sf)
			if moduleSym == nil {
				continue
			}
			exportSym := c.TryGetMemberInModuleExportsAndProperties(memberName, moduleSym)
			if exportSym == nil {
				continue
			}
			exportSym = resolveAliasedSymbol(c, exportSym)
			if symbolsMatch(c, exportSym, sym) {
				return true
			}
		}
	}

	return false
}
