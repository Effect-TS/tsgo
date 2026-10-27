package rules

import "github.com/effect-ts/tsgo/internal/typeparser"

// hasExplicitTypeArguments reports whether a piping flow step is called with
// explicit type arguments, which a rewrite to another API cannot carry over.
func hasExplicitTypeArguments(transformation *typeparser.PipingFlowTransformation) bool {
	return transformation != nil && transformation.TypeArguments() != nil && len(transformation.TypeArguments().Nodes) > 0
}

// isPipingFlowArgumentKind reports whether a step is written as an argument of
// a pipe, pipeable or Effect.fn call rather than applied to its input directly.
func isPipingFlowArgumentKind(kind typeparser.TransformationKind) bool {
	switch kind {
	case typeparser.TransformationKindPipe, typeparser.TransformationKindPipeable,
		typeparser.TransformationKindEffectFn, typeparser.TransformationKindEffectFnUntraced:
		return true
	}
	return false
}
