package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// EffectYieldableType resolves both plain Effect types and yieldable wrappers
// that implement the asEffect() protocol.
// For v3: delegates directly to EffectType (v3 models yieldable through Effect subtyping).
// For v4: tries EffectType first; if that fails, looks for an asEffect property,
// checks if it's callable, and tries EffectType on the return type of each call signature.
// Returns nil if the type is not an Effect and not yieldable.
func (tp *TypeParser) EffectYieldableType(t *checker.Type) *Effect {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}
	return Cached(&tp.links.EffectYieldableType, t, func() *Effect {
		version := tp.DetectEffectVersion()

		// For v3, yieldable types are modeled through Effect subtyping,
		// so EffectType alone is sufficient.
		if version != EffectMajorV4 {
			return tp.EffectType(t)
		}

		// v4: first try plain Effect type
		if result := tp.EffectType(t); result != nil {
			return result
		}

		// v4: look for asEffect() protocol
		asEffectType := tp.GetTypeOfPropertyByName(t, "asEffect")
		if asEffectType == nil {
			return nil
		}

		signatures := tp.checker.GetSignaturesOfType(asEffectType, checker.SignatureKindCall)
		for _, sig := range signatures {
			returnType := tp.checker.GetReturnTypeOfSignature(sig)
			if returnType == nil {
				continue
			}
			if result := tp.EffectType(returnType); result != nil {
				return result
			}
		}

		return nil
	})
}

// EffectYieldableTypeAtLocation resolves the type of node as an Effect or a
// yieldable wrapper (see EffectYieldableType). Returns nil if the type is
// neither, without computing the type when nodeCannotBeEffectLibraryType rules
// node out. Object literals are never ruled out: in v4 an object literal with
// an asEffect member is yieldable without any "~effect/..." key.
func (tp *TypeParser) EffectYieldableTypeAtLocation(node *ast.Node) *Effect {
	if tp == nil || node == nil || (node.Kind != ast.KindObjectLiteralExpression && tp.nodeCannotBeEffectLibraryType(node)) {
		return nil
	}
	return tp.EffectYieldableType(tp.GetTypeAtLocation(node))
}
