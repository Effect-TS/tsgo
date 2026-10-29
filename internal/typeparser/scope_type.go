package typeparser

import (
	"strings"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// ScopeTypeId is the property key for Scope's variance struct.
const ScopeTypeId = "~effect/Scope"

// ScopeType returns t itself when it is an Effect Scope type, or nil otherwise.
// For v4, this checks for the "~effect/Scope" computed property.
// For v3/unknown, this checks that the type is "pipeable" (has a callable pipe property)
// and that any required non-optional property's symbol name contains "ScopeTypeId".
func (tp *TypeParser) ScopeType(t *checker.Type) *checker.Type {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}
	return Cached(&tp.links.ScopeType, t, func() *checker.Type {
		version := tp.DetectEffectVersion()
		if version == EffectMajorV4 {
			if tp.GetTypeOfPropertyByName(t, ScopeTypeId) == nil {
				return nil
			}
			return t
		}

		// v3 / unknown: check that the type is "pipeable"
		pipeType := tp.GetTypeOfPropertyByName(t, "pipe")
		if pipeType == nil {
			return nil
		}
		signatures := tp.checker.GetSignaturesOfType(pipeType, checker.SignatureKindCall)
		if len(signatures) == 0 {
			return nil
		}

		// Check if any required non-optional property's symbol name contains "ScopeTypeId"
		for _, prop := range tp.checker.GetPropertiesOfType(t) {
			if prop == nil {
				continue
			}
			if ast.TSGetSymbolFlags(prop)&ast.SymbolFlagsProperty == 0 {
				continue
			}
			if ast.TSGetSymbolFlags(prop)&ast.SymbolFlagsOptional != 0 {
				continue
			}
			if ast.TSGetSymbolValueDeclaration(prop) == nil {
				continue
			}
			if strings.Contains(ast.TSGetSymbolName(prop), "ScopeTypeId") {
				return t
			}
		}

		return nil
	})
}
