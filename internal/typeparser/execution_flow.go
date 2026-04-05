// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/effect-ts/tsgo/internal/graph"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
)

type ExecutionNodeKind string

const (
	ExecutionNodeKindValue     ExecutionNodeKind = "value"
	ExecutionNodeKindTransform ExecutionNodeKind = "transform"
)

type ExecutionNode struct {
	Kind ExecutionNodeKind
	Node *ast.Node
	Type *checker.Type

	// Transform nodes preserve the original AST in Node and optionally expose a
	// normalized callee/args view once the visitor reaches that node.
	Callee *ast.Node
	Args   []*ast.Node
}

type ExecutionLinkKind string

const (
	ExecutionLinkKindUnknown   ExecutionLinkKind = "unknown"
	ExecutionLinkKindPipe      ExecutionLinkKind = "pipe"
	ExecutionLinkKindPipeable  ExecutionLinkKind = "pipeable"
	ExecutionLinkKindEffectFn  ExecutionLinkKind = "effectFn"
	ExecutionLinkKindCall      ExecutionLinkKind = "call"
	ExecutionLinkKindDataFirst ExecutionLinkKind = "dataFirst"
	ExecutionLinkKindDataLast  ExecutionLinkKind = "dataLast"
	ExecutionLinkKindFnPipe    ExecutionLinkKind = "fnPipe"
)

type ExecutionLink struct {
	Kind ExecutionLinkKind
	Node *ast.Node
}

type ExecutionFlow = graph.Graph[ExecutionNode, ExecutionLink]

func (tp *TypeParser) ExecutionFlow(sf *ast.SourceFile) *ExecutionFlow {
	if tp == nil || tp.checker == nil || sf == nil {
		return nil
	}

	return Cached(&tp.links.ExecutionFlow, sf, func() *ExecutionFlow {
		g := graph.New[ExecutionNode, ExecutionLink]()

		var parentStartingNode core.LinkStore[*ast.Node, *graph.NodeIndex]
		var expectFillInPipeTransformInfo core.LinkStore[*ast.Node, *graph.NodeIndex]

		var walk ast.Visitor
		walk = func(node *ast.Node) bool {
			if node == nil {
				return false
			}
			var newTrailingNode *graph.NodeIndex

			if expectFillInPipeTransformInfo.Has(node) {
				transformNodeIndex := *expectFillInPipeTransformInfo.TryGet(node)
				if transformNodeIndex != nil {
					switch node.Kind {
					case ast.KindParenthesizedExpression:
						// (Effect.asVoid)
						*expectFillInPipeTransformInfo.Get(node.Expression()) = transformNodeIndex
						// ... possibly other types as well?
					case ast.KindCallExpression:
						// Effect.flatMap(...)
						g.UpdateNode(*transformNodeIndex, func(current ExecutionNode) ExecutionNode {
							current.Callee = node.Expression()
							current.Args = node.Arguments()
							return current
						})
					default:
						// Effect.asVoid
						g.UpdateNode(*transformNodeIndex, func(current ExecutionNode) ExecutionNode {
							current.Callee = node
							return current
						})
					}
				}
			}

			if parsedPipeCall := tp.ParsePipeCall(node); parsedPipeCall != nil {
				// this is a pipe call, so we have subject and args
				subjectExecutionNode := g.AddNode(ExecutionNode{
					Kind: ExecutionNodeKindValue,
					Node: parsedPipeCall.Subject,
					Type: parsedPipeCall.SubjectType,
				})
				*parentStartingNode.Get(parsedPipeCall.Subject) = &subjectExecutionNode
				// and then we connect the args
				lastNode := subjectExecutionNode
				for i, pipeTransformNode := range parsedPipeCall.Args {
					transformExecInfo := ExecutionNode{
						Kind: ExecutionNodeKindTransform,
						Node: pipeTransformNode,
						Type: parsedPipeCall.ArgsOutType[i],
					}
					transformNode := g.AddNode(transformExecInfo)
					kind := ExecutionLinkKindPipeable
					if parsedPipeCall.Kind == TransformationKindPipe {
						kind = ExecutionLinkKindPipe
					}
					g.AddEdge(lastNode, transformNode, ExecutionLink{
						Kind: kind,
						Node: node,
					})
					lastNode = transformNode
					*expectFillInPipeTransformInfo.Get(pipeTransformNode) = &transformNode
				}
				newTrailingNode = &lastNode
			} else if dataFirstLastCall := tp.ParseDataFirstCallAsPipeable(node); dataFirstLastCall != nil {
				// this is a pipe call, so we have subject and args
				subjectExecutionNode := g.AddNode(ExecutionNode{
					Kind: ExecutionNodeKindValue,
					Node: dataFirstLastCall.Subject,
					Type: tp.GetTypeAtLocation(dataFirstLastCall.Subject),
				})
				*parentStartingNode.Get(dataFirstLastCall.Subject) = &subjectExecutionNode
				// transform
				transformExecInfo := ExecutionNode{
					Kind:   ExecutionNodeKindTransform,
					Node:   node,
					Type:   tp.GetTypeAtLocation(node),
					Callee: dataFirstLastCall.Callee,
					Args:   dataFirstLastCall.Args,
				}
				transformNode := g.AddNode(transformExecInfo)
				kind := ExecutionLinkKindDataFirst
				if dataFirstLastCall.SubjectIndex != 0 {
					kind = ExecutionLinkKindDataLast
				}
				g.AddEdge(subjectExecutionNode, transformNode, ExecutionLink{
					Kind: kind,
					Node: node,
				})
				newTrailingNode = &transformNode
			} else if fnCall := tp.EffectFnCall(node); fnCall != nil {
				// this is a pipe call, so we have subject and args
				subjectExecutionNode := g.AddNode(ExecutionNode{
					Kind: ExecutionNodeKindValue,
					Node: fnCall.FunctionNode,
					Type: nil, // TODO: get return type of function
				})
				// and then we connect the args
				lastNode := subjectExecutionNode
				for i, fnPipeNode := range fnCall.PipeArguments {
					var outType *checker.Type
					if i < len(fnCall.PipeArgsOutType) {
						outType = fnCall.PipeArgsOutType[i]
					}
					transformExecInfo := ExecutionNode{
						Kind: ExecutionNodeKindTransform,
						Node: fnPipeNode,
						Type: outType,
					}
					transformNode := g.AddNode(transformExecInfo)
					kind := ExecutionLinkKindFnPipe
					g.AddEdge(lastNode, transformNode, ExecutionLink{
						Kind: kind,
						Node: node,
					})
					lastNode = transformNode
					*expectFillInPipeTransformInfo.Get(fnPipeNode) = &transformNode
				}
			}

			// if there was a parent starting flow, we replace that with the last transformation
			if newTrailingNode != nil && parentStartingNode.Has(node) {
				parentStartingNodeIndex := *parentStartingNode.TryGet(node)
				if parentStartingNodeIndex != nil {
					for _, edgeIndex := range g.OutgoingEdges(*parentStartingNodeIndex) {
						edge, _ := g.GetEdge(edgeIndex)
						g.AddEdge(*newTrailingNode, edge.Target, edge.Data)
						g.RemoveEdge(edgeIndex)
					}
					g.RemoveNode(*parentStartingNodeIndex)
				}
			}
			node.ForEachChild(walk)
			return false
		}

		walk(sf.AsNode())
		return g
	})
}
