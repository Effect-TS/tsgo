package etsgoapi

import (
	"github.com/effect-ts/tsgo/internal/readonly"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// TransformationKind represents how a transformation was expressed in source code.
type TransformationKind string

const (
	TransformationKindPipe              = TransformationKind(typeparser.TransformationKindPipe)
	TransformationKindPipeable          = TransformationKind(typeparser.TransformationKindPipeable)
	TransformationKindDataFirst         = TransformationKind(typeparser.TransformationKindDataFirst)
	TransformationKindDataLast          = TransformationKind(typeparser.TransformationKindDataLast)
	TransformationKindCall              = TransformationKind(typeparser.TransformationKindCall)
	TransformationKindCallWithOptionals = TransformationKind(typeparser.TransformationKindCallWithOptionals)
	TransformationKindEffectFn          = TransformationKind(typeparser.TransformationKindEffectFn)
	TransformationKindEffectFnUntraced  = TransformationKind(typeparser.TransformationKindEffectFnUntraced)
)

// PipingFlowTransformation represents a single transformation step in a piping flow.
type PipingFlowTransformation struct {
	kind          TransformationKind        // How the transformation was expressed
	callee        *ast.Node                 // The function being applied (e.g., Effect.map)
	typeArguments *ast.NodeList             // Explicit type arguments to the transformation call, if any
	args          readonly.Slice[*ast.Node] // Arguments to the transformation, or nil for constants/single-arg calls
	outType       *checker.Type             // The resulting type after this transformation (may be nil)
}

func (r PipingFlowTransformation) Kind() TransformationKind { return r.kind }

func (r PipingFlowTransformation) Callee() *ast.Node { return r.callee }

func (r PipingFlowTransformation) TypeArguments() *ast.NodeList { return r.typeArguments }

func (r PipingFlowTransformation) Args() readonly.Slice[*ast.Node] { return r.args }

func (r PipingFlowTransformation) OutType() *checker.Type { return r.outType }

// PipingFlowSubject is the starting expression of a piping flow.
type PipingFlowSubject struct {
	node    *ast.Node     // The expression node
	outType *checker.Type // The type of the subject expression (may be nil)
}

func (r PipingFlowSubject) Node() *ast.Node { return r.node }

func (r PipingFlowSubject) OutType() *checker.Type { return r.outType }

// PipingFlow represents a complete piping flow: a subject followed by transformations.
type PipingFlow struct {
	node            *ast.Node                  // The outermost expression encompassing the entire flow
	subject         PipingFlowSubject          // The starting expression and its type
	transformations []PipingFlowTransformation // Ordered list of transformations
}

func (r PipingFlow) Node() *ast.Node { return r.node }

func (r PipingFlow) Subject() PipingFlowSubject { return r.subject }

func (r PipingFlow) Transformations() readonly.Slice[PipingFlowTransformation] {
	return readonly.New(r.transformations)
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

func pipingFlowsFromInternal(flows readonly.Slice[*typeparser.PipingFlow]) []*PipingFlow {
	if flows.IsNil() {
		return nil
	}
	result := make([]*PipingFlow, 0, flows.Len())
	for _, flow := range flows.All() {
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

	transformations := make([]PipingFlowTransformation, 0, flow.Transformations().Len())
	for _, transformation := range flow.Transformations().All() {
		transformations = append(transformations, PipingFlowTransformation{
			kind:          TransformationKind(transformation.Kind()),
			callee:        transformation.Callee(),
			typeArguments: transformation.TypeArguments(),
			args:          transformation.Args(),
			outType:       transformation.OutType(),
		})
	}

	return &PipingFlow{
		node: flow.Node(),
		subject: PipingFlowSubject{
			node:    flow.Subject().Node(),
			outType: flow.Subject().OutType(),
		},
		transformations: transformations,
	}
}

// TransformationInputType returns the type immediately before the indexed step.
// Missing types and indices outside the flow return nil.
func (flow *PipingFlow) TransformationInputType(index int) *checker.Type {
	if flow == nil || index < 0 || index >= len(flow.transformations) {
		return nil
	}
	if index == 0 {
		return flow.subject.outType
	}
	return flow.transformations[index-1].outType
}
