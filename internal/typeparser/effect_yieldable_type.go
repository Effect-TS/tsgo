package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// EffectYieldableType resolves both plain Effect types and yieldable wrappers
// (Option, Either, etc.) that implement the asEffect() protocol.
// For v3: delegates directly to EffectType (v3 models yieldable through Effect subtyping).
// For v4: tries EffectType first; if that fails, looks for an asEffect property,
// checks if it's callable, and tries EffectType on the return type of each call signature.
// Returns nil if the type is not an Effect and not yieldable.
func (tp *TypeParser) EffectYieldableType(t *checker.Type, atLocation *ast.Node) *Effect {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}
	return Cached(&tp.links.EffectYieldableType, t, func() *Effect {
		version := tp.DetectEffectVersion()

		// For v3, yieldable types are modeled through Effect subtyping,
		// so EffectType alone is sufficient.
		if version != EffectMajorV4 {
			return tp.EffectType(t, atLocation)
		}

		// v4: first try plain Effect type
		if result := tp.EffectType(t, atLocation); result != nil {
			return result
		}

		// v4: look for asEffect() protocol
		asEffectSymbol := tp.GetPropertyOfTypeByName(t, "asEffect")
		if asEffectSymbol == nil {
			return nil
		}

		asEffectType := tp.checker.GetTypeOfSymbolAtLocation(asEffectSymbol, atLocation)
		if asEffectType == nil {
			return nil
		}

		signatures := tp.checker.GetSignaturesOfType(asEffectType, checker.SignatureKindCall)
		for _, sig := range signatures {
			returnType := tp.checker.GetReturnTypeOfSignature(sig)
			if returnType == nil {
				continue
			}
			if result := tp.EffectType(returnType, atLocation); result != nil {
				return result
			}
		}

		return nil
	})
}
