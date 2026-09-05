package etsgoapi

import (
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// TransformationKind represents how a transformation was expressed in source code.
type TransformationKind string

const (
	TransformationKindPipe             = TransformationKind(typeparser.TransformationKindPipe)
	TransformationKindPipeable         = TransformationKind(typeparser.TransformationKindPipeable)
	TransformationKindDataFirst        = TransformationKind(typeparser.TransformationKindDataFirst)
	TransformationKindDataLast         = TransformationKind(typeparser.TransformationKindDataLast)
	TransformationKindCall             = TransformationKind(typeparser.TransformationKindCall)
	TransformationKindEffectFn         = TransformationKind(typeparser.TransformationKindEffectFn)
	TransformationKindEffectFnUntraced = TransformationKind(typeparser.TransformationKindEffectFnUntraced)
)

// PipingFlowTransformation represents a single transformation step in a piping flow.
type PipingFlowTransformation struct {
	Kind          TransformationKind // How the transformation was expressed
	Callee        *ast.Node          // The function being applied (e.g., Effect.map)
	TypeArguments *ast.NodeList      // Explicit type arguments to the transformation call, if any
	Args          []*ast.Node        // Arguments to the transformation, or nil for constants/single-arg calls
	OutType       *checker.Type      // The resulting type after this transformation (may be nil)
}

// PipingFlowSubject is the starting expression of a piping flow.
type PipingFlowSubject struct {
	Node    *ast.Node     // The expression node
	OutType *checker.Type // The type of the subject expression (may be nil)
}

// PipingFlow represents a complete piping flow: a subject followed by transformations.
type PipingFlow struct {
	Node            *ast.Node                  // The outermost expression encompassing the entire flow
	Subject         PipingFlowSubject          // The starting expression and its type
	Transformations []PipingFlowTransformation // Ordered list of transformations
}

// PipingFlows returns all piping flows found in a source file, sorted by source position.
// Each source transformation occurrence belongs to at most one returned flow.
// When includeEffectFn is true, Effect.fn / Effect.fnUntraced calls carrying trailing
// transformation arguments are surfaced as flows as well.
func (tp *TypeParser) PipingFlows(sf *ast.SourceFile, includeEffectFn bool) []*PipingFlow {
	if tp == nil || tp.inner == nil {
		return nil
	}
	return pipingFlowsFromInternal(tp.inner.PipingFlows(sf, includeEffectFn))
}

// LongestPipingFlowAt returns the longest normalized piping flow rooted at node,
// without returning a larger flow that merely encloses node.
func (tp *TypeParser) LongestPipingFlowAt(node *ast.Node, includeEffectFn bool) *PipingFlow {
	if tp == nil || tp.inner == nil {
		return nil
	}
	return pipingFlowFromInternal(tp.inner.LongestPipingFlowAt(node, includeEffectFn))
}

func pipingFlowsFromInternal(flows []*typeparser.PipingFlow) []*PipingFlow {
	if flows == nil {
		return nil
	}
	result := make([]*PipingFlow, 0, len(flows))
	for _, flow := range flows {
		if converted := pipingFlowFromInternal(flow); converted != nil {
			result = append(result, converted)
		}
	}
	return result
}

func pipingFlowFromInternal(flow *typeparser.PipingFlow) *PipingFlow {
	if flow == nil {
		return nil
	}

	transformations := make([]PipingFlowTransformation, 0, len(flow.Transformations))
	for _, transformation := range flow.Transformations {
		transformations = append(transformations, PipingFlowTransformation{
			Kind:          TransformationKind(transformation.Kind),
			Callee:        transformation.Callee,
			TypeArguments: transformation.TypeArguments,
			Args:          transformation.Args,
			OutType:       transformation.OutType,
		})
	}

	return &PipingFlow{
		Node: flow.Node,
		Subject: PipingFlowSubject{
			Node:    flow.Subject.Node,
			OutType: flow.Subject.OutType,
		},
		Transformations: transformations,
	}
}
