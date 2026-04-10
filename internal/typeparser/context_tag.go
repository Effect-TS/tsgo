package typeparser

import (
	"sort"
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// ContextTag parses a v3 Context.Tag type and extracts Identifier, Shape parameters.
// Returns nil if the type is not a v3 Context.Tag.
func (tp *TypeParser) ContextTag(t *checker.Type, atLocation *ast.Node) *Service {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}
	return Cached(&tp.links.ContextTag, t, func() *Service {
		if tp.DetectEffectVersion() != EffectMajorV3 {
			return nil
		}
		if !tp.IsPipeableType(t, atLocation) {
			return nil
		}

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

		sort.SliceStable(candidates, func(i, j int) bool {
			iHas := strings.Contains(candidates[i].Name, "TypeId")
			jHas := strings.Contains(candidates[j].Name, "TypeId")
			if iHas && !jHas {
				return true
			}
			return false
		})

		for _, prop := range candidates {
			propType := tp.checker.GetTypeOfSymbolAtLocation(prop, atLocation)
			if result := tp.parseServiceVarianceStruct(propType, atLocation); result != nil {
				return result
			}
		}

		return nil
	})
}

// IsContextTag returns true if the type has the Context.Tag variance struct.
func (tp *TypeParser) IsContextTag(t *checker.Type, atLocation *ast.Node) bool {
	return tp.ContextTag(t, atLocation) != nil
}
