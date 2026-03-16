package typeparser

import (
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// IsScopeType returns true if the type is an Effect Scope type.
// For v4, this checks for the "~effect/Scope" computed property.
// For v3/unknown, this checks that the type is "pipeable" (has a callable pipe property)
// and that any required non-optional property's symbol name contains "ScopeTypeId".
func IsScopeType(c *checker.Checker, t *checker.Type, atLocation *ast.Node) bool {
	if c == nil || t == nil {
		return false
	}
	links := GetEffectLinks(c)
	return Cached(&links.IsScopeType, t, func() bool {
		version := DetectEffectVersion(c)
		if version == EffectMajorV4 {
			return GetPropertyOfTypeByName(c, t, ScopeTypeId) != nil
		}

		// v3 / unknown: check that the type is "pipeable"
		pipeSymbol := c.GetPropertyOfType(t, "pipe")
		if pipeSymbol == nil {
			return false
		}
		pipeType := c.GetTypeOfSymbolAtLocation(pipeSymbol, atLocation)
		signatures := c.GetSignaturesOfType(pipeType, checker.SignatureKindCall)
		if len(signatures) == 0 {
			return false
		}

		// Check if any required non-optional property's symbol name contains "ScopeTypeId"
		for _, prop := range c.GetPropertiesOfType(t) {
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
			if strings.Contains(prop.Name, "ScopeTypeId") {
				return true
			}
		}

		return false
	})
}
