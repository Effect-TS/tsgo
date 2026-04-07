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
	ExecutionNodeKindFunction   ExecutionNodeKind = "function"
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
	ExecutionLinkKindUnknown         ExecutionLinkKind = "unknown"
	ExecutionLinkKindConnect         ExecutionLinkKind = "connect"
	ExecutionLinkKindPipe            ExecutionLinkKind = "pipe"
	ExecutionLinkKindPipeable        ExecutionLinkKind = "pipeable"
	ExecutionLinkKindEffectFn        ExecutionLinkKind = "effectFn"
	ExecutionLinkKindCall            ExecutionLinkKind = "call"
	ExecutionLinkKindDataFirst       ExecutionLinkKind = "dataFirst"
	ExecutionLinkKindDataLast        ExecutionLinkKind = "dataLast"
	ExecutionLinkKindFnPipe          ExecutionLinkKind = "fnPipe"
	ExecutionLinkKindPotentialReturn ExecutionLinkKind = "potentialReturn"
	ExecutionLinkKindParameter       ExecutionLinkKind = "parameter"
	ExecutionLinkKindTransformArg    ExecutionLinkKind = "transformArg"
	ExecutionLinkKindTransformCallee ExecutionLinkKind = "transformCallee"
)

type ExecutionLink struct {
	Kind ExecutionLinkKind
	Node *ast.Node
}

type (
	ExecutionFlow = graph.Graph[ExecutionNode, ExecutionLink]
	GraphSlice    struct {
		Leading  *graph.NodeIndex
		Trailing *graph.NodeIndex
	}
)

func (tp *TypeParser) ExecutionFlow(sf *ast.SourceFile) *ExecutionFlow {
	if tp == nil || tp.checker == nil || sf == nil {
		return nil
	}

	// TODO: calls like Layer.succeed(FileSystem)(arg) that are transforms
	// TODO: special effect generator handling

	return Cached(&tp.links.ExecutionFlow, sf, func() *ExecutionFlow {
		g := graph.New[ExecutionNode, ExecutionLink]()

		var connectTrailingOfNodeToMap core.LinkStore[*ast.Node, *graph.NodeIndex]
		var attemptFillCalleeAndArgs core.LinkStore[*ast.Node, *graph.NodeIndex]
		var extraFunctionMiddleware core.LinkStore[*ast.Node, GraphSlice]
		var isEffectYieldingGenerator core.LinkStore[*ast.Node, bool]
		var valueExecNodeByNode core.LinkStore[*ast.Node, graph.NodeIndex]

		NewExecValueNode := func(node *ast.Node) graph.NodeIndex {
			maybeIdx := valueExecNodeByNode.TryGet(node)
			if maybeIdx != nil {
				return *maybeIdx
			}
			newIdx := g.AddNode(ExecutionNode{
				Kind: ExecutionNodeKindValue,
				Node: node,
				Type: tp.GetTypeAtLocation(node),
			})
			*valueExecNodeByNode.Get(node) = newIdx
			return newIdx
		}

		PrepareCalleeAndArgs := func(execNode *graph.NodeIndex) bool {
			g.UpdateNode(*execNode, func(node ExecutionNode) ExecutionNode {
				if node.Callee != nil {
					calleeIdx := NewExecValueNode(node.Callee)
					g.AddEdge(calleeIdx, *execNode, ExecutionLink{
						Kind: ExecutionLinkKindTransformCallee,
					})
				}
				if node.Args != nil {
					for _, arg := range node.Args {
						argIdx := NewExecValueNode(arg)
						g.AddEdge(argIdx, *execNode, ExecutionLink{
							Kind: ExecutionLinkKindTransformArg,
						})
						*connectTrailingOfNodeToMap.Get(arg) = &argIdx
					}
				}
				return node
			})
			return true
		}

		NewPipeTransformSlice := func(initialNode *graph.NodeIndex, linkKind ExecutionLinkKind, nodes []*ast.Node, types []*checker.Type) (*graph.NodeIndex, *graph.NodeIndex) {
			lastNode := initialNode
			firstNode := initialNode
			for i, arg := range nodes {
				transformIndex := g.AddNode(ExecutionNode{
					Kind: ExecutionNodeKindTransform,
					Type: types[i],
					Node: arg,
				})
				if lastNode != nil {
					g.AddEdge(*lastNode, transformIndex, ExecutionLink{
						Kind: linkKind,
					})
				}
				if firstNode == nil {
					firstNode = &transformIndex
				}
				lastNode = &transformIndex
				*attemptFillCalleeAndArgs.Get(arg) = &transformIndex
			}
			return firstNode, lastNode
		}

		ConnectTrailingNodeToParentLeading := func(node *ast.Node, newTrailingNode *graph.NodeIndex) {
			if connectTrailingOfNodeToMap.Has(node) {
				parentStartingNodeIndex := *connectTrailingOfNodeToMap.TryGet(node)
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
		}

		var walk ast.Visitor
		walk = func(node *ast.Node) bool {
			if node == nil {
				return false
			}

			// a parent node may have injected a transformation, and we need to set the callee and args
			if attemptFillCalleeAndArgs.Has(node) {
				transformNodeIndex := *attemptFillCalleeAndArgs.TryGet(node)
				if transformNodeIndex != nil {
					switch node.Kind {
					case ast.KindParenthesizedExpression:
						// (Effect.asVoid)
						*attemptFillCalleeAndArgs.Get(node.Expression()) = transformNodeIndex
						// ... possibly other types as well?
					case ast.KindCallExpression:
						// Effect.flatMap(...)
						g.UpdateNode(*transformNodeIndex, func(current ExecutionNode) ExecutionNode {
							current.Callee = node.Expression()
							current.Args = node.Arguments()
							return current
						})
						PrepareCalleeAndArgs(transformNodeIndex)
					default:
						// Effect.asVoid
						g.UpdateNode(*transformNodeIndex, func(current ExecutionNode) ExecutionNode {
							current.Callee = node
							return current
						})
						PrepareCalleeAndArgs(transformNodeIndex)
					}
				}
			}

			if fnCall := tp.EffectFnCall(node); fnCall != nil {
				if fnCall.GeneratorFunction() != nil {
					*isEffectYieldingGenerator.Get(fnCall.FunctionNode) = true
				}
				first, last := NewPipeTransformSlice(
					nil,
					ExecutionLinkKindFnPipe,
					fnCall.PipeArguments,
					fnCall.PipeArgsOutType)
				if last != nil {
					*extraFunctionMiddleware.Get(fnCall.FunctionNode.AsNode()) = GraphSlice{
						Leading:  first,
						Trailing: last,
					}
				}
				if connectTrailingOfNodeToMap.Has(node) {
					*connectTrailingOfNodeToMap.Get(fnCall.FunctionNode.AsNode()) = *connectTrailingOfNodeToMap.Get(node)
				}
			} else if effectGen := tp.EffectGenCall(node); effectGen != nil {
				*isEffectYieldingGenerator.Get(effectGen.GeneratorFunction.AsNode()) = true
			} else if parsedPipeCall := tp.ParsePipeCall(node); parsedPipeCall != nil {
				// this is a pipe call, so we have subject and args
				subjectExecutionNode := NewExecValueNode(parsedPipeCall.Subject)
				*connectTrailingOfNodeToMap.Get(parsedPipeCall.Subject) = &subjectExecutionNode
				// and then we connect the args
				_, last := NewPipeTransformSlice(
					&subjectExecutionNode,
					ExecutionLinkKindPipe,
					parsedPipeCall.Args,
					parsedPipeCall.ArgsOutType)
				ConnectTrailingNodeToParentLeading(node, last)
			} else if dataFirstLastCall := tp.DataFirstOrLastCall(node); dataFirstLastCall != nil {
				// this is a pipe call, so we have subject and args
				subjectExecutionNode := NewExecValueNode(dataFirstLastCall.Subject)
				*connectTrailingOfNodeToMap.Get(dataFirstLastCall.Subject) = &subjectExecutionNode
				// transform
				transformNode := g.AddNode(ExecutionNode{
					Kind:   ExecutionNodeKindTransform,
					Node:   node,
					Type:   tp.GetTypeAtLocation(node),
					Callee: dataFirstLastCall.Callee,
					Args:   dataFirstLastCall.Args,
				})
				kind := ExecutionLinkKindDataFirst
				if dataFirstLastCall.SubjectIndex != 0 {
					kind = ExecutionLinkKindDataLast
				}
				g.AddEdge(subjectExecutionNode, transformNode, ExecutionLink{
					Kind: kind,
					Node: node,
				})
				PrepareCalleeAndArgs(&transformNode)
				ConnectTrailingNodeToParentLeading(node, &transformNode)
			} else if ast.IsFunctionLikeDeclaration(node) && !isEffectYieldingGenerator.Has(node) {
				fnLikeData := node.FunctionLikeData()
				fnExecNode := g.AddNode(ExecutionNode{
					Kind: ExecutionNodeKindFunction,
					Node: node,
					Type: tp.GetTypeAtLocation(node),
				})
				for _, par := range fnLikeData.Parameters.Nodes {
					parNode := NewExecValueNode(par)
					g.AddEdge(parNode, fnExecNode, ExecutionLink{
						Kind: ExecutionLinkKindParameter,
					})
				}
				// body is a simple expression (arrow function)
				fnBody := node.Body()
				var returnNodes []graph.NodeIndex
				if fnBody != nil && ast.IsArrowFunction(node) && ast.IsExpressionNode(fnBody) {
					bodyNode := node.AsArrowFunction().Body
					returnExprNode := NewExecValueNode(bodyNode)
					*connectTrailingOfNodeToMap.Get(bodyNode) = &returnExprNode
					returnNodes = append(returnNodes, returnExprNode)
				} else {
					ast.ForEachReturnStatement(fnBody, func(node *ast.Node) bool {
						if node.Kind == ast.KindReturnStatement {
							returnedExpr := node.AsReturnStatement().Expression
							if returnedExpr != nil {
								returnIndex := NewExecValueNode(returnedExpr)
								*connectTrailingOfNodeToMap.Get(returnedExpr) = &returnIndex
								returnNodes = append(returnNodes, returnIndex)
							}
						}
						return false
					})
				}
				if extraFunctionMiddleware.Has(node) {
					// we have middleware, connect all to a logic merge, and then add middleware, and then fnNode
					mergeReturn := g.AddNode(ExecutionNode{
						Kind: ExecutionNodeKindLogicMerge,
						Node: node,
						Type: nil, // TODO: return type of function
					})
					for _, n := range returnNodes {
						g.AddEdge(n, mergeReturn, ExecutionLink{
							Kind: ExecutionLinkKindPotentialReturn,
						})
					}
					middleware := extraFunctionMiddleware.Get(node)
					g.AddEdge(mergeReturn, *middleware.Leading, ExecutionLink{
						Kind: ExecutionLinkKindFnPipe,
					})
					g.AddEdge(*middleware.Trailing, fnExecNode, ExecutionLink{
						Kind: ExecutionLinkKindPotentialReturn,
					})
				} else {
					// no middleware, result jumps directly to function node
					for _, n := range returnNodes {
						g.AddEdge(n, fnExecNode, ExecutionLink{
							Kind: ExecutionLinkKindPotentialReturn,
						})
					}
				}
				ConnectTrailingNodeToParentLeading(node, &fnExecNode)
			}

			node.ForEachChild(walk)
			return false
		}

		walk(sf.AsNode())
		return g
	})
}
