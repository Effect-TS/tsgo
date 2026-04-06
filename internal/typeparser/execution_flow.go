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
	ExecutionNodeKindValue      ExecutionNodeKind = "value"
	ExecutionNodeKindLogicMerge ExecutionNodeKind = "logicMerge"
	ExecutionNodeKindTransform  ExecutionNodeKind = "transform"
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
	ExecutionLinkKindConnect   ExecutionLinkKind = "connect"
	ExecutionLinkKindPipe      ExecutionLinkKind = "pipe"
	ExecutionLinkKindPipeable  ExecutionLinkKind = "pipeable"
	ExecutionLinkKindEffectFn  ExecutionLinkKind = "effectFn"
	ExecutionLinkKindCall      ExecutionLinkKind = "call"
	ExecutionLinkKindDataFirst ExecutionLinkKind = "dataFirst"
	ExecutionLinkKindDataLast  ExecutionLinkKind = "dataLast"
	ExecutionLinkKindFnPipe    ExecutionLinkKind = "fnPipe"
	ExecutionLinkKindReturn    ExecutionLinkKind = "return"
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
			} else if dataFirstLastCall := tp.DataFirstOrLastCall(node); dataFirstLastCall != nil {
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
				var lastNode graph.NodeIndex
				bodyNode := fnCall.Body()
				handled := false
				if ast.IsExpressionNode(bodyNode) && fnCall.GeneratorFunction() == nil {
					// we have an arrow function with an expression
					lastNode = g.AddNode(ExecutionNode{
						Kind: ExecutionNodeKindValue,
						Node: bodyNode,
						Type: tp.GetTypeAtLocation(bodyNode),
					})
					handled = true
				} else if bodyNode.Kind == ast.KindBlock && fnCall.GeneratorFunction() == nil {
					// we have a regular function with multiple potential exit points
					lastNode = g.AddNode(ExecutionNode{
						Kind: ExecutionNodeKindLogicMerge,
						Node: fnCall.FunctionNode,
						Type: nil, // TODO
					})
					ast.ForEachReturnStatement(bodyNode, func(node *ast.Node) bool {
						if node.Kind == ast.KindReturnStatement {
							returnedExpr := node.AsReturnStatement().Expression
							if returnedExpr != nil {
								returnIndex := g.AddNode(ExecutionNode{
									Kind: ExecutionNodeKindValue,
									Node: returnedExpr,
									Type: tp.GetTypeAtLocation(returnedExpr),
								})
								g.AddEdge(returnIndex, lastNode, ExecutionLink{
									Kind: ExecutionLinkKindReturn,
								})
								*parentStartingNode.Get(returnedExpr) = &returnIndex
							}
						}
						return false
					})
					handled = true
				}
				if handled {
					// and then we connect the args
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
			}

			// if there was a parent starting value point, re replace that with a merge, and point last node to that merge
			if newTrailingNode != nil && parentStartingNode.Has(node) {
				parentStartingNodeIndex := *parentStartingNode.TryGet(node)
				g.UpdateNode(*parentStartingNodeIndex, func(node ExecutionNode) ExecutionNode {
					if node.Kind == ExecutionNodeKindValue {
						node.Kind = ExecutionNodeKindLogicMerge
					}
					if node.Kind == ExecutionNodeKindLogicMerge {
						g.AddEdge(*newTrailingNode, *parentStartingNodeIndex, ExecutionLink{
							Kind: ExecutionLinkKindConnect,
						})
					}
					return node
				})
			}
			node.ForEachChild(walk)
			return false
		}

		walk(sf.AsNode())
		return g
	})
}
