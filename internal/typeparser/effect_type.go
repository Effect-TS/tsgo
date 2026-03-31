package typeparser

import (
	"sort"
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

var effectModuleDescriptor = PackageSourceFileDescriptor{
	PackageName:       "effect",
	MatchesSourceFile: isEffectTypeSourceFile,
}

var effectPackageExportDescriptor = PackageSourceFileDescriptor{
	PackageName: "effect",
}

// EffectTypeId is the property key for Effect's variance struct.
// Effect v4 (effect-smol) uses this pattern to encode type parameters.
const EffectTypeId = "~effect/Effect"

// Effect represents parsed Effect<A, E, R> type parameters.
type Effect struct {
	A *checker.Type // Success type
	E *checker.Type // Error type
	R *checker.Type // Requirements type
}

// EffectType parses an Effect type and extracts A, E, R parameters.
// Returns nil if the type is not an Effect.
// The detection strategy is chosen based on the detected Effect version:
// v4 uses direct symbol lookup, v3/unknown uses property iteration.
func (tp *TypeParser) EffectType(t *checker.Type, atLocation *ast.Node) *Effect {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}
	return Cached(&tp.links.EffectType, t, func() *Effect {
		version := tp.DetectEffectVersion()
		if version == EffectMajorV4 {
			// Direct property access using the known Effect v4 type ID
			propSymbol := tp.GetPropertyOfTypeByName(t, EffectTypeId)
			if propSymbol == nil {
				return nil
			}

			// Get the variance struct type
			varianceStructType := tp.checker.GetTypeOfSymbolAtLocation(propSymbol, atLocation)

			// Parse the variance struct to extract A, E, R
			return tp.parseVarianceStruct(varianceStructType, atLocation)
		}

		// v3 / unknown: iterate properties looking for a variance struct
		props := tp.checker.GetPropertiesOfType(t)

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
			propType := tp.checker.GetTypeOfSymbolAtLocation(prop, atLocation)
			if result := tp.parseVarianceStruct(propType, atLocation); result != nil {
				return result
			}
		}

		return nil
	})
}

// parseVarianceStruct extracts A, E, R from a variance struct type.
func (tp *TypeParser) parseVarianceStruct(t *checker.Type, atLocation *ast.Node) *Effect {
	a := tp.extractCovariantType(t, atLocation, "_A")
	if a == nil {
		return nil
	}

	e := tp.extractCovariantType(t, atLocation, "_E")
	if e == nil {
		return nil
	}

	r := tp.extractCovariantType(t, atLocation, "_R")
	if r == nil {
		return nil
	}

	return &Effect{A: a, E: e, R: r}
}

// IsEffectType returns true if the type has the Effect variance struct.
func (tp *TypeParser) IsEffectType(t *checker.Type, atLocation *ast.Node) bool {
	return tp.EffectType(t, atLocation) != nil
}

// StrictEffectType returns the parsed Effect type only if the type's symbol name
// is "Effect". This filters out types like Stream, Layer, HttpApp.Default that
// carry the variance struct but are not Effect itself.
func (tp *TypeParser) StrictEffectType(t *checker.Type, atLocation *ast.Node) *Effect {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}
	return Cached(&tp.links.StrictEffectType, t, func() *Effect {
		result := tp.EffectType(t, atLocation)
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
func (tp *TypeParser) StrictIsEffectType(t *checker.Type, atLocation *ast.Node) bool {
	return tp.StrictEffectType(t, atLocation) != nil
}

// EffectSubtype detects types that have the Effect variance struct AND a "_tag" or "get"
// marker property (e.g., Exit, Option, Either, Pool). Returns nil if not an Effect subtype.
func (tp *TypeParser) EffectSubtype(t *checker.Type, atLocation *ast.Node) *Effect {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}
	return Cached(&tp.links.EffectSubtype, t, func() *Effect {
		// Check for "_tag" or "get" property first (quick rejection)
		tagSymbol := tp.checker.GetPropertyOfType(t, "_tag")
		getSymbol := tp.checker.GetPropertyOfType(t, "get")
		if tagSymbol == nil && getSymbol == nil {
			return nil
		}
		// Must also be an Effect type
		return tp.EffectType(t, atLocation)
	})
}

// IsEffectSubtype returns true if the type is an Effect subtype (has variance struct + "_tag" or "get").
func (tp *TypeParser) IsEffectSubtype(t *checker.Type, atLocation *ast.Node) bool {
	return tp.EffectSubtype(t, atLocation) != nil
}

// FiberType detects types that have the Effect variance struct AND both "await" and "poll"
// properties. Returns nil if the type is not a Fiber.
func (tp *TypeParser) FiberType(t *checker.Type, atLocation *ast.Node) *Effect {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}
	return Cached(&tp.links.FiberType, t, func() *Effect {
		// Check for both "await" and "poll" properties (quick rejection)
		awaitSymbol := tp.checker.GetPropertyOfType(t, "await")
		pollSymbol := tp.checker.GetPropertyOfType(t, "poll")
		if awaitSymbol == nil || pollSymbol == nil {
			return nil
		}
		// Must also be an Effect type
		return tp.EffectType(t, atLocation)
	})
}

// IsFiberType returns true if the type is a Fiber type (has variance struct + "await" and "poll").
func (tp *TypeParser) IsFiberType(t *checker.Type, atLocation *ast.Node) bool {
	return tp.FiberType(t, atLocation) != nil
}

// HasEffectTypeId returns true if the type has the Effect type identifier.
// For v4, this is a quick check for the "~effect/Effect" property.
// For v3/unknown, this defers to IsEffectType since there is no single property shortcut.
func (tp *TypeParser) HasEffectTypeId(t *checker.Type, atLocation *ast.Node) bool {
	if tp == nil || tp.checker == nil || t == nil {
		return false
	}
	return Cached(&tp.links.HasEffectTypeId, t, func() bool {
		version := tp.DetectEffectVersion()
		if version == EffectMajorV4 {
			return tp.GetPropertyOfTypeByName(t, EffectTypeId) != nil
		}
		// For v3/unknown, the quick check is not available; defer to full detection.
		return tp.IsEffectType(t, atLocation)
	})
}

func isEffectTypeSourceFile(tp *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
	if c == nil || sf == nil {
		return false
	}

	moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
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

	return tp.EffectType(effectType, sf.AsNode()) != nil
}

// IsExpressionEffectModule reports whether node resolves to the Effect module namespace
// (e.g., the `Effect` in `import { Effect } from "effect"`).
func (tp *TypeParser) IsExpressionEffectModule(node *ast.Node) bool {
	return tp.IsNodeReferenceToModule(node, effectModuleDescriptor)
}

// IsNodeReferenceToEffectModuleApi reports whether node resolves to a member exported by the "effect" package.
func (tp *TypeParser) IsNodeReferenceToEffectModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectModuleDescriptor, memberName)
}

// IsNodeReferenceToEffectPackageExport reports whether node resolves to a member
// exported by any module in the "effect" npm package. Unlike IsNodeReferenceToEffectModuleApi,
// this does not require the source file to export the Effect type — it only checks
// that the declaration lives inside the "effect" package and matches the named export.
// This is needed for functions like `pipe` which are exported from `effect/Function`.
func (tp *TypeParser) IsNodeReferenceToEffectPackageExport(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectPackageExportDescriptor, memberName)
}
