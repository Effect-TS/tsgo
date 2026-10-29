package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// StreamTypeId is the property key for Stream's variance struct.
const StreamTypeId = "~effect/Stream"

// StreamType parses a Stream type and extracts A, E, R parameters.
// For v3, Stream carries the Effect variance struct, so this delegates to EffectType.
func (tp *TypeParser) StreamType(t *checker.Type) *Effect {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}
	return Cached(&tp.links.StreamType, t, func() *Effect {
		if tp.DetectEffectVersion() != EffectMajorV4 {
			return tp.EffectType(t)
		}

		varianceStructType := tp.GetTypeOfPropertyByName(t, StreamTypeId)
		if varianceStructType == nil {
			return nil
		}
		return tp.parseVarianceStruct(varianceStructType)
	})
}

// StreamTypeAtLocation parses the type of node as a Stream type.
// Returns nil if the type is not a Stream, without computing the type when
// nodeCannotBeEffectLibraryType rules node out.
func (tp *TypeParser) StreamTypeAtLocation(node *ast.Node) *Effect {
	return tp.StreamType(tp.effectLibraryTypeAtLocation(node))
}
