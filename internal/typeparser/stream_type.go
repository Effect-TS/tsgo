package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// StreamTypeId is the property key for Stream's variance struct.
const StreamTypeId = "~effect/Stream"

// StreamType parses a Stream type and extracts A, E, R parameters.
// For v3, Stream carries the Effect variance struct, so this delegates to EffectType.
func (tp *TypeParser) StreamType(t *checker.Type, atLocation *ast.Node) *Effect {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}
	return Cached(&tp.links.StreamType, t, func() *Effect {
		if tp.DetectEffectVersion() != EffectMajorV4 {
			return tp.EffectType(t, atLocation)
		}

		propSymbol := tp.GetPropertyOfTypeByName(t, StreamTypeId)
		if propSymbol == nil {
			return nil
		}

		varianceStructType := tp.checker.GetTypeOfSymbolAtLocation(propSymbol, atLocation)
		return tp.parseVarianceStruct(varianceStructType, atLocation)
	})
}
