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
	ExecutionLinkKindYieldable       ExecutionLinkKind = "yieldable"
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
		var parsedEffectGenForNode core.LinkStore[*ast.Node, EffectGenCallResult]
		var parsedEffectFnForNode core.LinkStore[*ast.Node, EffectFnCallResult]
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
				if newTrailingNode != parentStartingNodeIndex {
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
		}

		ConnectYieldStarInBody := func(node *ast.Node, toGraphNode *graph.NodeIndex) {
			checker.ForEachYieldExpression(node, func(expr *ast.Node) bool {
				if expr != nil && expr.Expression() != nil {
					valueNode := NewExecValueNode(expr.Expression())
					*connectTrailingOfNodeToMap.Get(expr.Expression()) = &valueNode
					g.AddEdge(valueNode, *toGraphNode, ExecutionLink{
						Kind: ExecutionLinkKindYieldable,
					})
				}
				return false
			})
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
					*parsedEffectFnForNode.Get(fnCall.FunctionNode) = *fnCall
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
				// we mark the function node to be a gen and to connect to this node in the end
				*parsedEffectGenForNode.Get(effectGen.GeneratorFunction.AsNode()) = *effectGen
				if connectTrailingOfNodeToMap.Has(node) {
					*connectTrailingOfNodeToMap.Get(effectGen.GeneratorFunction.AsNode()) = *connectTrailingOfNodeToMap.Get(node)
				}
			} else if parsedInlinePipeableCall := tp.singleArgInlineCall(node); parsedInlinePipeableCall != nil {
				// this is a Layer.succeed(FileSystem)(arg) where Layer.succeed(FileSystem) has only 1 sig, with 1 arg
				subjectExecutionNode := NewExecValueNode(parsedInlinePipeableCall.Subject)
				*connectTrailingOfNodeToMap.Get(parsedInlinePipeableCall.Subject) = &subjectExecutionNode
				_, last := NewPipeTransformSlice(
					&subjectExecutionNode,
					ExecutionLinkKindPipe,
					[]*ast.Node{parsedInlinePipeableCall.Transform},
					[]*checker.Type{tp.GetTypeAtLocation(node)},
				)
				ConnectTrailingNodeToParentLeading(node, last)
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
			} else if parsedEffectGenForNode.Has(node) {
				effectGenInfo := parsedEffectGenForNode.Get(node)
				if effectGenInfo != nil {
					mergeNode := g.AddNode(ExecutionNode{
						Kind: ExecutionNodeKindLogicMerge,
						Type: effectGenInfo.FunctionReturnType,
						Node: node,
					})
					ConnectYieldStarInBody(effectGenInfo.Body, &mergeNode)
					ConnectTrailingNodeToParentLeading(node, &mergeNode)
				}
			} else if ast.IsFunctionLikeDeclaration(node) {
				// regular function
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

				fnBody := node.Body()
				var fnExitNode *graph.NodeIndex
				if fnBody != nil && ast.IsArrowFunction(node) && ast.IsExpressionNode(fnBody) {
					// this is just an (...args) => expr, so exit node is expr itself
					bodyNode := node.AsArrowFunction().Body
					exitNode := NewExecValueNode(bodyNode)
					fnExitNode = &exitNode
					*connectTrailingOfNodeToMap.Get(bodyNode) = fnExitNode
				} else if fnBody != nil {
					if parsedEffectFnForNode.Has(node) {
						parsedEffectFn := parsedEffectFnForNode.Get(node)
						exitNode := g.AddNode(ExecutionNode{
							Kind: ExecutionNodeKindLogicMerge,
							Type: parsedEffectFn.FunctionReturnType,
							Node: node,
						})
						fnExitNode = &exitNode
						if parsedEffectFn.IsGenerator() {
							ConnectYieldStarInBody(fnBody, &exitNode)
						}
					} else {
						declSig := tp.checker.GetSignatureFromDeclaration(node)
						if declSig != nil {
							retType := tp.checker.GetReturnTypeOfSignature(declSig)
							if retType != nil {
								exitNode := g.AddNode(ExecutionNode{
									Kind: ExecutionNodeKindLogicMerge,
									Type: retType,
									Node: node,
								})
								fnExitNode = &exitNode
							}
						}
					}
					ast.ForEachReturnStatement(fnBody, func(node *ast.Node) bool {
						if node.Kind == ast.KindReturnStatement {
							returnedExpr := node.AsReturnStatement().Expression
							if returnedExpr != nil && fnExitNode != nil {
								returnIndex := NewExecValueNode(returnedExpr)
								*connectTrailingOfNodeToMap.Get(returnedExpr) = &returnIndex
								g.AddEdge(returnIndex, *fnExitNode, ExecutionLink{
									Kind: ExecutionLinkKindPotentialReturn,
								})
							}
						}
						return false
					})
				}

				if fnExitNode != nil {
					if extraFunctionMiddleware.Has(node) {
						// we have middleware, connect all to a logic merge, and then add middleware, and then fnNode
						middleware := extraFunctionMiddleware.Get(node)
						if middleware != nil && middleware.Leading != nil && middleware.Trailing != nil && middleware.Leading != middleware.Trailing {
							g.AddEdge(*fnExitNode, *middleware.Leading, ExecutionLink{
								Kind: ExecutionLinkKindFnPipe,
							})
							fnExitNode = middleware.Trailing
						}
					}
					g.AddEdge(*fnExitNode, fnExecNode, ExecutionLink{
						Kind: ExecutionLinkKindPotentialReturn,
					})
					ConnectTrailingNodeToParentLeading(node, &fnExecNode)
				}
			}

			node.ForEachChild(walk)
			return false
		}

		walk(sf.AsNode())

		/*
			// cleanup step, we can remove  --- only one connect ---> LogicMerge
			for idx, node := range g.Nodes() {
				// we are a merge
				if node.Kind != ExecutionNodeKindLogicMerge {
					continue
				}
				// with only 1 incoming node
				incomingEdges := g.IncomingEdges(idx)
				if len(incomingEdges) != 1 {
					continue
				}
				edge, ok := g.GetEdge(incomingEdges[0])
				if !ok {
					continue
				}
				// and we are a connect
				if edge.Data.Kind != ExecutionLinkKindConnect {
					continue
				}
				sourceNode, sourceOk := g.GetNode(edge.Source)
				if !sourceOk {
					continue
				}
				// the source has same node and type as this one
				if sourceNode.Node != node.Node || sourceNode.Type != node.Type {
					continue
				}
				// we proceed by reconnecting all of the outgoing edges to the edge source directly
				for outEdgeIdx := range g.OutgoingEdges(idx) {
					outEdge, okOut := g.GetEdge(outEdgeIdx)
					if okOut {
						g.AddEdge(edge.Source, outEdge.Target, outEdge.Data)
					}
				}
				// and we remove ourself
				g.RemoveEdge(incomingEdges[0])
				g.RemoveNode(idx)
			}*/

		return g
	})
}

type parsedSingleArgInlineCallTransform struct {
	Subject   *ast.Node
	Transform *ast.Node
}

func (tp *TypeParser) singleArgInlineCall(node *ast.Node) *parsedSingleArgInlineCallTransform {
	if node == nil {
		return nil
	}
	if node.Kind != ast.KindCallExpression {
		return nil
	}
	outerCallExpr := node.AsCallExpression()
	if outerCallExpr.Expression == nil {
		return nil
	}
	outerCallArgs := node.Arguments()
	if len(outerCallArgs) != 1 {
		return nil
	}
	calledExprType := tp.GetTypeAtLocation(outerCallExpr.Expression)
	if calledExprType == nil {
		return nil
	}
	callSigs := tp.checker.GetCallSignatures(calledExprType)
	if len(callSigs) != 1 {
		return nil
	}
	params := callSigs[0].Parameters()
	if len(params) != 1 {
		return nil
	}

	return &parsedSingleArgInlineCallTransform{
		Subject:   outerCallArgs[0],
		Transform: outerCallExpr.Expression,
	}
}
