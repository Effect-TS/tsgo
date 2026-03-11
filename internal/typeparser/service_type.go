package typeparser

import (
	"sort"
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// parseServiceVarianceStruct extracts Identifier and Shape from a Service variance struct type.
func parseServiceVarianceStruct(c *checker.Checker, t *checker.Type, atLocation *ast.Node) *Service {
	identifier := extractInvariantType(c, t, atLocation, "_Identifier")
	if identifier == nil {
		return nil
	}

	shape := extractInvariantType(c, t, atLocation, "_Service")
	if shape == nil {
		return nil
	}

	return &Service{Identifier: identifier, Shape: shape}
}

// ServiceType parses a Service type and extracts Identifier, Shape parameters.
// Returns nil if the type is not a Service.
func ServiceType(c *checker.Checker, t *checker.Type, atLocation *ast.Node) *Service {
	propSymbol := GetPropertyOfTypeByName(c, t, ServiceTypeId)
	if propSymbol == nil {
		return nil
	}

	varianceStructType := c.GetTypeOfSymbolAtLocation(propSymbol, atLocation)

	return parseServiceVarianceStruct(c, varianceStructType, atLocation)
}

// IsServiceType returns true if the type has the Service variance struct.
func IsServiceType(c *checker.Checker, t *checker.Type, atLocation *ast.Node) bool {
	return ServiceType(c, t, atLocation) != nil
}

// ContextTag parses a Context.Tag type and extracts Identifier, Shape parameters.
// Returns nil if the type is not a Context.Tag.
// For V4, this delegates to ServiceType() since both resolve to the same type ID.
// For V3/unknown, this iterates properties looking for a service variance struct,
// following the same pattern as LayerType() and EffectType().
func ContextTag(c *checker.Checker, t *checker.Type, atLocation *ast.Node) *Service {
	version := DetectEffectVersion(c)
	if version == EffectMajorV4 {
		return ServiceType(c, t, atLocation)
	}

	// v3 / unknown: iterate properties looking for a service variance struct
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
		propType := c.GetTypeOfSymbolAtLocation(prop, atLocation)
		if result := parseServiceVarianceStruct(c, propType, atLocation); result != nil {
			return result
		}
	}

	return nil
}

// IsContextTag returns true if the type has the Context.Tag variance struct.
func IsContextTag(c *checker.Checker, t *checker.Type, atLocation *ast.Node) bool {
	return ContextTag(c, t, atLocation) != nil
}
