package typeparser

import (
	"sort"
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

var effectSchemaModuleDescriptor = PackageSourceFileDescriptor{
	PackageName:       "effect",
	MatchesSourceFile: isSchemaTypeSourceFile,
}

var effectParseResultModuleDescriptor = PackageSourceFileDescriptor{
	PackageName:       "effect",
	MatchesSourceFile: isParseResultSourceFile,
}

var effectSchemaParserModuleDescriptor = PackageSourceFileDescriptor{
	PackageName:       "effect",
	MatchesSourceFile: isSchemaParserSourceFile,
}

// parseSchemaVarianceStruct checks if a type is a Schema variance struct (has _A, _I, _R).
func (tp *TypeParser) parseSchemaVarianceStruct(t *checker.Type, atLocation *ast.Node) bool {
	a := tp.extractInvariantType(t, atLocation, "_A")
	if a == nil {
		return false
	}
	i := tp.extractInvariantType(t, atLocation, "_I")
	if i == nil {
		return false
	}
	r := tp.extractCovariantType(t, atLocation, "_R")
	return r != nil
}

// isSchemaType checks if a type is a Schema type (v4 or v3).
func (tp *TypeParser) isSchemaType(t *checker.Type, atLocation *ast.Node) bool {
	c := tp.checker
	if c == nil || t == nil {
		return false
	}
	return Cached(&tp.links.IsSchemaType, t, func() bool {
		version := tp.DetectEffectVersion()
		if version == EffectMajorV4 {
			return tp.GetPropertyOfTypeByName(t, SchemaTypeId) != nil
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
			if tp.parseSchemaVarianceStruct(propType, atLocation) {
				return true
			}
		}

		return false
	})
}

// IsSchemaType returns true if the type is a Schema type (v4 or v3).
func (tp *TypeParser) IsSchemaType(t *checker.Type, atLocation *ast.Node) bool {
	if tp == nil {
		return false
	}
	return tp.isSchemaType(t, atLocation)
}

// SchemaTypes holds the A (Type) and E (Encoded) types extracted from a Schema type.
type SchemaTypes struct {
	A *checker.Type
	E *checker.Type
}

// EffectSchemaTypes extracts the A (Type) and E (Encoded) types from a Schema type.
// Returns nil if the type is not a recognized Schema type or types cannot be extracted.
func (tp *TypeParser) EffectSchemaTypes(t *checker.Type, atLocation *ast.Node) *SchemaTypes {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}
	c := tp.checker
	return Cached(&tp.links.EffectSchemaTypes, t, func() *SchemaTypes {
		version := tp.DetectEffectVersion()
		if version == EffectMajorV4 {
			if tp.GetPropertyOfTypeByName(t, SchemaTypeId) == nil {
				return nil
			}
			// V4: get Type and Encoded properties directly
			aType := tp.getPropertyType(t, atLocation, "Type")
			eType := tp.getPropertyType(t, atLocation, "Encoded")
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
			a := tp.extractInvariantType(propType, atLocation, "_A")
			if a == nil {
				continue
			}
			i := tp.extractInvariantType(propType, atLocation, "_I")
			if i == nil {
				continue
			}
			r := tp.extractCovariantType(propType, atLocation, "_R")
			if r == nil {
				continue
			}
			return &SchemaTypes{A: a, E: i}
		}

		return nil
	})
}

// getPropertyType extracts the type of a named property from a type.
func (tp *TypeParser) getPropertyType(t *checker.Type, atLocation *ast.Node, propName string) *checker.Type {
	c := tp.checker
	sym := c.GetPropertyOfType(t, propName)
	if sym == nil {
		return nil
	}
	return c.GetTypeOfSymbolAtLocation(sym, atLocation)
}

func isSchemaTypeSourceFile(tp *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
	if c == nil || sf == nil {
		return false
	}

	moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
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

	return tp.isSchemaType(schemaType, sf.AsNode())
}

// IsNodeReferenceToEffectSchemaModuleApi reports whether node resolves to a member
// exported by the "effect" package from a module that exports the Schema type.
func (tp *TypeParser) IsNodeReferenceToEffectSchemaModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectSchemaModuleDescriptor, memberName)
}

func isParseResultSourceFile(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
	if c == nil || sf == nil {
		return false
	}

	moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
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
func (tp *TypeParser) IsNodeReferenceToEffectParseResultModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectParseResultModuleDescriptor, memberName)
}

func isSchemaParserSourceFile(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
	if c == nil || sf == nil {
		return false
	}

	moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
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
func (tp *TypeParser) IsNodeReferenceToEffectSchemaParserModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectSchemaParserModuleDescriptor, memberName)
}
