// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
)

// nodeText extracts the source text of a node from the source file.
func nodeText(sf *ast.SourceFile, node *ast.Node) string {
	if node == nil || sf == nil {
		return ""
	}
	text := sf.Text()
	pos := node.Pos()
	end := node.End()
	if pos >= 0 && end >= pos && end <= len(text) {
		return text[pos:end]
	}
	return ""
}

// ReconstructPipingFlow reconstructs a piping flow into a string expression
// by applying transformations sequentially.
// For example: subject with transformations [f, g] becomes "g(f(subject))".
//
// Note: Effect.fn and Effect.fnUntraced transformations cannot be reconstructed
// as a chain since they are part of the Effect.fn call itself. In this case,
// the original subject node text is returned.
func ReconstructPipingFlow(sf *ast.SourceFile, subject *PipingFlowSubject, transformations []*PipingFlowTransformation) string {
	if sf == nil || subject == nil {
		return ""
	}

	// Check if all transformations are effectFn or effectFnUntraced.
	// In this case, reconstruction is not possible - return the original node text.
	if len(transformations) > 0 {
		allEffectFn := true
		for _, t := range transformations {
			if t.Kind != TransformationKindEffectFn && t.Kind != TransformationKindEffectFnUntraced {
				allEffectFn = false
				break
			}
		}
		if allEffectFn {
			return nodeText(sf, subject.Node)
		}
	}

	result := nodeText(sf, subject.Node)

	for _, t := range transformations {
		if t.Kind == TransformationKindEffectFn || t.Kind == TransformationKindEffectFnUntraced {
			// Effect.fn transformations cannot be reconstructed as part of a chain
			continue
		}

		calleeText := nodeText(sf, t.Callee)

		if t.Kind == TransformationKindCall {
			// Single-arg call: callee(result)
			result = calleeText + "(" + result + ")"
		} else {
			// Pipe or pipeable: apply the transformation
			if len(t.Args) > 0 {
				// Curried form: callee(args...)(result)
				var argsText strings.Builder
				for i, arg := range t.Args {
					if i > 0 {
						argsText.WriteString(", ")
					}
					argsText.WriteString(nodeText(sf, arg))
				}
				result = calleeText + "(" + argsText.String() + ")(" + result + ")"
			} else {
				// Constant: callee(result)
				result = calleeText + "(" + result + ")"
			}
		}
	}

	return result
}
