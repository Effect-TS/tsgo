package typeparser

import (
	"sort"
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// parseServiceVarianceStruct extracts Identifier and Shape from a Service variance struct type.
func (tp *TypeParser) parseServiceVarianceStruct(t *checker.Type, atLocation *ast.Node) *Service {
	identifier := tp.extractInvariantType(t, atLocation, "_Identifier")
	if identifier == nil {
		return nil
	}

	shape := tp.extractInvariantType(t, atLocation, "_Service")
	if shape == nil {
		return nil
	}

	return &Service{Identifier: identifier, Shape: shape}
}

// ServiceType parses a Service type and extracts Identifier, Shape parameters.
// Returns nil if the type is not a Service.
func (tp *TypeParser) ServiceType(t *checker.Type, atLocation *ast.Node) *Service {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}
	return Cached(&tp.links.ServiceType, t, func() *Service {
		propSymbol := tp.GetPropertyOfTypeByName(t, ServiceTypeId)
		if propSymbol == nil {
			return nil
		}

		varianceStructType := tp.checker.GetTypeOfSymbolAtLocation(propSymbol, atLocation)

		return tp.parseServiceVarianceStruct(varianceStructType, atLocation)
	})
}

// IsServiceType returns true if the type has the Service variance struct.
func (tp *TypeParser) IsServiceType(t *checker.Type, atLocation *ast.Node) bool {
	return tp.ServiceType(t, atLocation) != nil
}

// ContextTag parses a Context.Tag type and extracts Identifier, Shape parameters.
// Returns nil if the type is not a Context.Tag.
// For V4, this delegates to ServiceType() since both resolve to the same type ID.
// For V3/unknown, this iterates properties looking for a service variance struct,
// following the same pattern as LayerType() and EffectType().
func (tp *TypeParser) ContextTag(t *checker.Type, atLocation *ast.Node) *Service {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}
	return Cached(&tp.links.ContextTag, t, func() *Service {
		version := tp.DetectEffectVersion()
		if version == EffectMajorV4 {
			return tp.ServiceType(t, atLocation)
		}

		// v3 / unknown: iterate properties looking for a service variance struct
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

		// Sort so properties containing "TypeId" come first (optimization heuristic)
		sort.SliceStable(candidates, func(i, j int) bool {
			iHas := strings.Contains(candidates[i].Name, "TypeId")
			jHas := strings.Contains(candidates[j].Name, "TypeId")
			if iHas && !jHas {
				return true
			}
			return false
		})

		// Try each candidate as a service variance struct
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
