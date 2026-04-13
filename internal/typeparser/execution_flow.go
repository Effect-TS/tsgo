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
		var valueExecNodeByNode core.LinkStore[*ast.Node, graph.NodeIndex]
		var skipNodes core.LinkStore[*ast.Node, bool]

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
					*connectTrailingOfNodeToMap.Get(node.Callee) = &calleeIdx
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
				if *newTrailingNode != *parentStartingNodeIndex {
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
			if node != nil {
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
		}

		ConnectReturnInBody := func(node *ast.Node, toGraphNode *graph.NodeIndex) {
			if node != nil {
				ast.ForEachReturnStatement(node, func(node *ast.Node) bool {
					if node.Kind == ast.KindReturnStatement {
						returnedExpr := node.AsReturnStatement().Expression
						if returnedExpr != nil {
							returnIndex := NewExecValueNode(returnedExpr)
							*connectTrailingOfNodeToMap.Get(returnedExpr) = &returnIndex
							g.AddEdge(returnIndex, *toGraphNode, ExecutionLink{
								Kind: ExecutionLinkKindPotentialReturn,
							})
						}
					}
					return false
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

			if skipNodes.Has(node) {
				// noop, someone already handled this node somehow
			} else if fnCall := tp.EffectFnCall(node); fnCall != nil {
				// an Effect.fn is a special syntax for a function with pipe middleware and gen execution
				fnExecNode := g.AddNode(ExecutionNode{
					Kind: ExecutionNodeKindFunction,
					Type: tp.GetTypeAtLocation(node),
					Node: node,
				})
				fnExitNode := g.AddNode(ExecutionNode{
					Kind: ExecutionNodeKindLogicMerge,
					Type: fnCall.FunctionReturnType,
					Node: fnCall.FunctionNode,
				})
				fnBody := fnCall.Body()
				if fnCall.IsGenerator() {
					ConnectYieldStarInBody(fnBody, &fnExitNode)
				}
				// arrow functions return directly the expression, otherwise look for return in block
				if fnBody != nil && ast.IsExpressionNode(fnBody) {
					exprBody := NewExecValueNode(fnBody)
					*connectTrailingOfNodeToMap.Get(fnBody) = &exprBody
					g.AddEdge(exprBody, fnExitNode, ExecutionLink{
						Kind: ExecutionLinkKindPotentialReturn,
					})
				} else {
					ConnectReturnInBody(fnBody, &fnExitNode)
				}
				// Effect.fn has traling pipes
				_, last := NewPipeTransformSlice(
					&fnExitNode,
					ExecutionLinkKindFnPipe,
					fnCall.PipeArguments,
					fnCall.PipeArgsOutType)
				g.AddEdge(*last, fnExecNode, ExecutionLink{
					Kind: ExecutionLinkKindPotentialReturn,
				})
				// connect the parameters
				for _, par := range fnCall.FunctionNode.Parameters() {
					parNode := NewExecValueNode(par)
					*connectTrailingOfNodeToMap.Get(par) = &parNode
					g.AddEdge(parNode, fnExecNode, ExecutionLink{
						Kind: ExecutionLinkKindParameter,
					})
				}
				// finalize
				ConnectTrailingNodeToParentLeading(node, &fnExecNode)
				*skipNodes.Get(fnCall.FunctionNode) = true
			} else if effectGen := tp.EffectGenCall(node); effectGen != nil {
				// an Effect.gen is a special syntax for node effect expressions
				genNode := NewExecValueNode(node)
				ConnectYieldStarInBody(effectGen.Body, &genNode)
				ConnectReturnInBody(effectGen.Body, &genNode)
				ConnectTrailingNodeToParentLeading(node, &genNode)
				*skipNodes.Get(effectGen.GeneratorFunction.AsNode()) = true
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
			} else if ast.IsFunctionLikeDeclaration(node) {
				// regular function
				var retType *checker.Type
				declSig := tp.checker.GetSignatureFromDeclaration(node)
				if declSig != nil {
					retType = tp.checker.GetReturnTypeOfSignature(declSig)
				}
				fnExecNode := g.AddNode(ExecutionNode{
					Kind: ExecutionNodeKindFunction,
					Type: tp.GetTypeAtLocation(node),
					Node: node,
				})
				fnBody := node.Body()
				// arrow functions return directly the expression, otherwise look for return in block
				if fnBody != nil && ast.IsExpressionNode(fnBody) {
					exprBody := NewExecValueNode(fnBody)
					*connectTrailingOfNodeToMap.Get(fnBody) = &exprBody
					g.AddEdge(exprBody, fnExecNode, ExecutionLink{
						Kind: ExecutionLinkKindPotentialReturn,
					})
				} else {
					fnExitNode := g.AddNode(ExecutionNode{
						Kind: ExecutionNodeKindLogicMerge,
						Type: retType,
						Node: node,
					})
					g.AddEdge(fnExitNode, fnExecNode, ExecutionLink{
						Kind: ExecutionLinkKindPotentialReturn,
					})
					ConnectReturnInBody(fnBody, &fnExitNode)
				}
				// connect the parameters
				for _, par := range node.Parameters() {
					parNode := NewExecValueNode(par)
					*connectTrailingOfNodeToMap.Get(par) = &parNode
					g.AddEdge(parNode, fnExecNode, ExecutionLink{
						Kind: ExecutionLinkKindParameter,
					})
				}
				// finalize
				ConnectTrailingNodeToParentLeading(node, &fnExecNode)
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
