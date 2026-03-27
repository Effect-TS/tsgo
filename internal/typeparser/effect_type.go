package typeparser

import (
	"sort"
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// EffectType parses an Effect type and extracts A, E, R parameters.
// Returns nil if the type is not an Effect.
// The detection strategy is chosen based on the detected Effect version:
// v4 uses direct symbol lookup, v3/unknown uses property iteration.
func EffectType(c *checker.Checker, t *checker.Type, atLocation *ast.Node) *Effect {
	if c == nil || t == nil {
		return nil
	}
	links := GetEffectLinks(c)
	return Cached(&links.EffectType, t, func() *Effect {
		version := DetectEffectVersion(c)
		if version == EffectMajorV4 {
			// Direct property access using the known Effect v4 type ID
			propSymbol := GetPropertyOfTypeByName(c, t, EffectTypeId)
			if propSymbol == nil {
				return nil
			}

			// Get the variance struct type
			varianceStructType := c.GetTypeOfSymbolAtLocation(propSymbol, atLocation)

			// Parse the variance struct to extract A, E, R
			return parseVarianceStruct(c, varianceStructType, atLocation)
		}

		// v3 / unknown: iterate properties looking for a variance struct
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

		// Sort so properties containing "EffectTypeId" come first (optimization heuristic)
		sort.SliceStable(candidates, func(i, j int) bool {
			iHas := strings.Contains(candidates[i].Name, "EffectTypeId")
			jHas := strings.Contains(candidates[j].Name, "EffectTypeId")
			if iHas && !jHas {
				return true
			}
			return false
		})

		// Try each candidate as a variance struct
		for _, prop := range candidates {
			propType := c.GetTypeOfSymbolAtLocation(prop, atLocation)
			if result := parseVarianceStruct(c, propType, atLocation); result != nil {
				return result
			}
		}

		return nil
	})
}

// parseVarianceStruct extracts A, E, R from a variance struct type.
func parseVarianceStruct(c *checker.Checker, t *checker.Type, atLocation *ast.Node) *Effect {
	a := extractCovariantType(c, t, atLocation, "_A")
	if a == nil {
		return nil
	}

	e := extractCovariantType(c, t, atLocation, "_E")
	if e == nil {
		return nil
	}

	r := extractCovariantType(c, t, atLocation, "_R")
	if r == nil {
		return nil
	}

	return &Effect{A: a, E: e, R: r}
}

// IsEffectType returns true if the type has the Effect variance struct.
func IsEffectType(c *checker.Checker, t *checker.Type, atLocation *ast.Node) bool {
	return EffectType(c, t, atLocation) != nil
}

// StrictEffectType returns the parsed Effect type only if the type's symbol name
// is "Effect". This filters out types like Stream, Layer, HttpApp.Default that
// carry the variance struct but are not Effect itself.
func StrictEffectType(c *checker.Checker, t *checker.Type, atLocation *ast.Node) *Effect {
	if c == nil || t == nil {
		return nil
	}
	links := GetEffectLinks(c)
	return Cached(&links.StrictEffectType, t, func() *Effect {
		result := EffectType(c, t, atLocation)
		if result == nil {
			return nil
		}
		sym := t.Symbol()
		if sym == nil {
			return nil
		}
		if sym.Name != "Effect" {
			return nil
		}
		return result
	})
}

// StrictIsEffectType returns true if the type has the Effect variance struct
// AND the type's symbol name is "Effect". This filters out types like Stream,
// Layer, HttpApp.Default that carry the variance struct but are not Effect itself.
func StrictIsEffectType(c *checker.Checker, t *checker.Type, atLocation *ast.Node) bool {
	return StrictEffectType(c, t, atLocation) != nil
}

// EffectSubtype detects types that have the Effect variance struct AND a "_tag" or "get"
// marker property (e.g., Exit, Option, Either, Pool). Returns nil if not an Effect subtype.
func EffectSubtype(c *checker.Checker, t *checker.Type, atLocation *ast.Node) *Effect {
	if c == nil || t == nil {
		return nil
	}
	links := GetEffectLinks(c)
	return Cached(&links.EffectSubtype, t, func() *Effect {
		// Check for "_tag" or "get" property first (quick rejection)
		tagSymbol := c.GetPropertyOfType(t, "_tag")
		getSymbol := c.GetPropertyOfType(t, "get")
		if tagSymbol == nil && getSymbol == nil {
			return nil
		}
		// Must also be an Effect type
		return EffectType(c, t, atLocation)
	})
}

// IsEffectSubtype returns true if the type is an Effect subtype (has variance struct + "_tag" or "get").
func IsEffectSubtype(c *checker.Checker, t *checker.Type, atLocation *ast.Node) bool {
	return EffectSubtype(c, t, atLocation) != nil
}

// FiberType detects types that have the Effect variance struct AND both "await" and "poll"
// properties. Returns nil if the type is not a Fiber.
func FiberType(c *checker.Checker, t *checker.Type, atLocation *ast.Node) *Effect {
	if c == nil || t == nil {
		return nil
	}
	links := GetEffectLinks(c)
	return Cached(&links.FiberType, t, func() *Effect {
		// Check for both "await" and "poll" properties (quick rejection)
		awaitSymbol := c.GetPropertyOfType(t, "await")
		pollSymbol := c.GetPropertyOfType(t, "poll")
		if awaitSymbol == nil || pollSymbol == nil {
			return nil
		}
		// Must also be an Effect type
		return EffectType(c, t, atLocation)
	})
}

// IsFiberType returns true if the type is a Fiber type (has variance struct + "await" and "poll").
func IsFiberType(c *checker.Checker, t *checker.Type, atLocation *ast.Node) bool {
	return FiberType(c, t, atLocation) != nil
}

// HasEffectTypeId returns true if the type has the Effect type identifier.
// For v4, this is a quick check for the "~effect/Effect" property.
// For v3/unknown, this defers to IsEffectType since there is no single property shortcut.
func HasEffectTypeId(c *checker.Checker, t *checker.Type, atLocation *ast.Node) bool {
	if c == nil || t == nil {
		return false
	}
	links := GetEffectLinks(c)
	return Cached(&links.HasEffectTypeId, t, func() bool {
		version := DetectEffectVersion(c)
		if version == EffectMajorV4 {
			return GetPropertyOfTypeByName(c, t, EffectTypeId) != nil
		}
		// For v3/unknown, the quick check is not available; defer to full detection.
		return IsEffectType(c, t, atLocation)
	})
}

func isEffectTypeSourceFile(c *checker.Checker, sf *ast.SourceFile) bool {
	if c == nil || sf == nil {
		return false
	}

	moduleSym := moduleSymbolFromSourceFile(c, sf)
	if moduleSym == nil {
		return false
	}

	effectSym := c.TryGetMemberInModuleExportsAndProperties("Effect", moduleSym)
	if effectSym == nil {
		return false
	}

	effectType := c.GetDeclaredTypeOfSymbol(effectSym)
	if effectType == nil {
		return false
	}

	return EffectType(c, effectType, sf.AsNode()) != nil
}

// IsExpressionEffectModule reports whether node resolves to the Effect module namespace
// (e.g., the `Effect` in `import { Effect } from "effect"`).
func IsExpressionEffectModule(c *checker.Checker, node *ast.Node) bool {
	if c == nil || node == nil {
		return false
	}

	sym := c.GetSymbolAtLocation(node)
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
			if isEffectTypeSourceFile(c, sf) {
				return true
			}
		}
	}

	return false
}

// IsNodeReferenceToEffectModuleApi reports whether node resolves to a member exported by the "effect" package.
func IsNodeReferenceToEffectModuleApi(c *checker.Checker, node *ast.Node, memberName string) bool {
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
			if !isEffectTypeSourceFile(c, sf) {
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

// IsNodeReferenceToEffectPackageExport reports whether node resolves to a member
// exported by any module in the "effect" npm package. Unlike IsNodeReferenceToEffectModuleApi,
// this does not require the source file to export the Effect type — it only checks
// that the declaration lives inside the "effect" package and matches the named export.
// This is needed for functions like `pipe` which are exported from `effect/Function`.
func IsNodeReferenceToEffectPackageExport(c *checker.Checker, node *ast.Node, memberName string) bool {
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
