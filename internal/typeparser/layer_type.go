package typeparser

import (
	"sort"
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// parseLayerVarianceStruct extracts ROut, E, RIn from a Layer variance struct type.
func parseLayerVarianceStruct(c *checker.Checker, t *checker.Type, atLocation *ast.Node) *Layer {
	rOut := extractContravariantType(c, t, atLocation, "_ROut")
	if rOut == nil {
		return nil
	}

	e := extractCovariantType(c, t, atLocation, "_E")
	if e == nil {
		return nil
	}

	rIn := extractCovariantType(c, t, atLocation, "_RIn")
	if rIn == nil {
		return nil
	}

	return &Layer{ROut: rOut, E: e, RIn: rIn}
}

// LayerType parses a Layer type and extracts ROut, E, RIn parameters.
// Returns nil if the type is not a Layer.
// The detection strategy is chosen based on the detected Effect version:
// v4 uses direct symbol lookup, v3/unknown uses property iteration.
func LayerType(c *checker.Checker, t *checker.Type, atLocation *ast.Node) *Layer {
	if c == nil || t == nil {
		return nil
	}
	links := GetEffectLinks(c)
	return Cached(&links.LayerType, t, func() *Layer {
		version := DetectEffectVersion(c)
		if version == EffectMajorV4 {
			// Direct property access using the known Layer v4 type ID
			propSymbol := GetPropertyOfTypeByName(c, t, LayerTypeId)
			if propSymbol == nil {
				return nil
			}

			varianceStructType := c.GetTypeOfSymbolAtLocation(propSymbol, atLocation)

			return parseLayerVarianceStruct(c, varianceStructType, atLocation)
		}

		// v3 / unknown: iterate properties looking for a layer variance struct
		props := c.GetPropertiesOfType(t)

		// Filter to required, non-optional properties with a value declaration
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
			return nil
		}

		// Sort so properties containing "LayerTypeId" come first (optimization heuristic)
		sort.SliceStable(candidates, func(i, j int) bool {
			iHas := strings.Contains(candidates[i].Name, "LayerTypeId")
			jHas := strings.Contains(candidates[j].Name, "LayerTypeId")
			if iHas && !jHas {
				return true
			}
			return false
		})

		// Try each candidate as a layer variance struct
		for _, prop := range candidates {
			propType := c.GetTypeOfSymbolAtLocation(prop, atLocation)
			if result := parseLayerVarianceStruct(c, propType, atLocation); result != nil {
				return result
			}
		}

		return nil
	})
}

// IsLayerType returns true if the type has the Layer variance struct.
func IsLayerType(c *checker.Checker, t *checker.Type, atLocation *ast.Node) bool {
	return LayerType(c, t, atLocation) != nil
}

func isLayerTypeSourceFile(c *checker.Checker, sf *ast.SourceFile) bool {
	if c == nil || sf == nil {
		return false
	}

	moduleSym := moduleSymbolFromSourceFile(c, sf)
	if moduleSym == nil {
		return false
	}

	layerSym := c.TryGetMemberInModuleExportsAndProperties("Layer", moduleSym)
	if layerSym == nil {
		return false
	}

	layerType := c.GetDeclaredTypeOfSymbol(layerSym)
	if layerType == nil {
		return false
	}

	return LayerType(c, layerType, sf.AsNode()) != nil
}

// IsNodeReferenceToEffectLayerModuleApi reports whether node resolves to a member
// exported by the "effect" package from a module that exports the Layer type.
func IsNodeReferenceToEffectLayerModuleApi(c *checker.Checker, node *ast.Node, memberName string) bool {
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
			if !isLayerTypeSourceFile(c, sf) {
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
			if checker.Checker_getSymbolIfSameReference(c, exportSym, sym) != nil {
				return true
			}
		}
	}

	return false
}
