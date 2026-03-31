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
func (tp *TypeParser) IsScopeType(t *checker.Type, atLocation *ast.Node) bool {
	if tp == nil || tp.checker == nil || t == nil {
		return false
	}
	return Cached(&tp.links.IsScopeType, t, func() bool {
		version := tp.DetectEffectVersion()
		if version == EffectMajorV4 {
			return tp.GetPropertyOfTypeByName(t, ScopeTypeId) != nil
		}

		// v3 / unknown: check that the type is "pipeable"
		pipeSymbol := tp.checker.GetPropertyOfType(t, "pipe")
		if pipeSymbol == nil {
			return false
		}
		pipeType := tp.checker.GetTypeOfSymbolAtLocation(pipeSymbol, atLocation)
		signatures := tp.checker.GetSignaturesOfType(pipeType, checker.SignatureKindCall)
		if len(signatures) == 0 {
			return false
		}

		// Check if any required non-optional property's symbol name contains "ScopeTypeId"
		for _, prop := range tp.checker.GetPropertiesOfType(t) {
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
