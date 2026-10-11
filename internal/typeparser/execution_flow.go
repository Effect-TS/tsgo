// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/effect-ts/tsgo/internal/graph"
	"github.com/effect-ts/tsgo/internal/readonly"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
)

type ExecutionNodeKind string

const (
	ExecutionNodeKindValue      ExecutionNodeKind = "value"
	ExecutionNodeKindFunction   ExecutionNodeKind = "function"
	ExecutionNodeKindLogicMerge ExecutionNodeKind = "logicMerge"
	ExecutionNodeKindTransform  ExecutionNodeKind = "transform"
)

type ExecutionNode struct {
	kind ExecutionNodeKind
	node *ast.Node
	typ  *checker.Type

	// Transform nodes preserve the original AST in Node and optionally expose a
	// normalized callee/args view once the visitor reaches that node.
	callee *ast.Node
	args   []*ast.Node
}

func (r ExecutionNode) Kind() ExecutionNodeKind { return r.kind }

func (r ExecutionNode) Node() *ast.Node { return r.node }

func (r ExecutionNode) Type() *checker.Type { return r.typ }

func (r ExecutionNode) Callee() *ast.Node { return r.callee }

func (r ExecutionNode) Args() readonly.Slice[*ast.Node] { return readonly.New(r.args) }

type ExecutionLinkKind string

const (
	ExecutionLinkKindUsedBy          ExecutionLinkKind = "usedBy"
	ExecutionLinkKindPipe            ExecutionLinkKind = "pipe"
	ExecutionLinkKindPotentialReturn ExecutionLinkKind = "potentialReturn"
	ExecutionLinkKindYieldable       ExecutionLinkKind = "yieldable"
	ExecutionLinkKindParameter       ExecutionLinkKind = "parameter"
	ExecutionLinkKindTransformArg    ExecutionLinkKind = "transformArg"
	ExecutionLinkKindTransformCallee ExecutionLinkKind = "transformCallee"
)

type ExecutionLink struct {
	kind ExecutionLinkKind
	node *ast.Node
}

func (r ExecutionLink) Kind() ExecutionLinkKind { return r.kind }

func (r ExecutionLink) Node() *ast.Node { return r.node }

type (
	ExecutionFlow struct {
		graph *graph.Graph[ExecutionNode, ExecutionLink]
	}
	GraphSlice struct {
		Leading  *graph.NodeIndex
		Trailing *graph.NodeIndex
	}
)

func (flow *ExecutionFlow) ToMermaid(options graph.MermaidOptions[ExecutionNode, ExecutionLink]) string {
	return flow.graph.ToMermaid(options)
}

type executionCollector struct {
	tp          *TypeParser
	g           *graph.Graph[ExecutionNode, ExecutionLink]
	parsed      *core.LinkStore[*ast.Node, *GraphSlice]
	usageTarget *GraphSlice
}

func (ec *executionCollector) buildSlice(node ExecutionNode) *GraphSlice {
	nodeIndex := ec.g.AddNode(node)
	return &GraphSlice{
		Leading:  &nodeIndex,
		Trailing: &nodeIndex,
	}
}

func (ec *executionCollector) buildValueNode(node *ast.Node) *GraphSlice {
	return ec.buildSlice(ExecutionNode{
		kind: ExecutionNodeKindValue,
		node: node,
		typ:  ec.tp.GetTypeAtLocation(node),
	})
}

func (ec *executionCollector) extractCalleeAndArgs(node *ast.Node) (*ast.Node, []*ast.Node) {
	if ast.IsParenthesizedExpression(node) {
		return ec.extractCalleeAndArgs(node.Expression())
	} else if ast.IsCallExpression(node) {
		return node.Expression(), node.Arguments()
	}
	return node, nil
}

func (ec *executionCollector) connectSlices(fromSlice *GraphSlice, toSlice *GraphSlice, kind ExecutionLinkKind) *GraphSlice {
	if fromSlice == nil {
		return toSlice
	}
	if toSlice == nil {
		return fromSlice
	}
	if *fromSlice.Trailing == *toSlice.Leading {
		return &GraphSlice{
			Leading:  fromSlice.Leading,
			Trailing: toSlice.Trailing,
		}
	}
	ec.g.AddEdge(*fromSlice.Trailing, *toSlice.Leading, ExecutionLink{
		kind: kind,
	})
	return &GraphSlice{
		Leading:  fromSlice.Leading,
		Trailing: toSlice.Trailing,
	}
}

func (ec *executionCollector) visitNode(node *ast.Node) *GraphSlice {
	// avoid double traversal
	if node == nil {
		return nil
	}
	if ec.parsed.Has(node) {
		return *ec.parsed.TryGet(node)
	}

	previousUsageTarget := ec.usageTarget
	ec.usageTarget = nil

	// actual visit logic
	var s *GraphSlice
	if parsedEffectGen := ec.tp.EffectGenCall(node); parsedEffectGen != nil {
		s = ec.visitEffectGenCall(parsedEffectGen, node)
	} else if parsedEffectFn := ec.tp.EffectFnCall(node); parsedEffectFn != nil {
		s = ec.visitEffectFnCall(parsedEffectFn, node)
	} else if parsedPipeCall := ec.tp.ParsePipeCall(node); parsedPipeCall != nil {
		s = ec.visitPipeCall(parsedPipeCall, node)
	} else if parsedSingleArg := ec.tp.singleArgInlineCall(node); parsedSingleArg != nil {
		s = ec.visitSingleArgInlineCall(parsedSingleArg, node)
	} else if parsedDataFirstOrLast := ec.tp.DataFirstOrLastCall(node); parsedDataFirstOrLast != nil {
		s = ec.visitDataFirstOrLastCall(parsedDataFirstOrLast, node)
	} else if ast.IsFunctionLikeDeclaration(node) {
		s = ec.visitFunctionLikeDeclaration(node)
	} else {
		s = ec.visitExpressionNode(node, previousUsageTarget)
	}
	// store to avoid double-traversal
	*ec.parsed.Get(node) = s
	ec.usageTarget = previousUsageTarget

	return s
}

func (ec *executionCollector) visitNodesAndConnectSlice(nodes []*ast.Node, targetSlice *GraphSlice, kind ExecutionLinkKind) bool {
	for _, n := range nodes {
		ec.visitNodeAndConnectSlice(n, targetSlice, kind)
	}
	return false
}

func (ec *executionCollector) visitNodeAndConnectSlice(n *ast.Node, targetSlice *GraphSlice, kind ExecutionLinkKind) bool {
	nodeSlice := ec.visitNode(n)
	ec.connectSlices(nodeSlice, targetSlice, kind)
	return false
}

func (ec *executionCollector) visitNodeVisitorConnectUsageTarget(node *ast.Node) bool {
	if node == nil {
		return false
	}
	s := ec.visitNode(node)
	ec.connectSlices(s, ec.usageTarget, ExecutionLinkKindUsedBy)
	return false
}

func (ec *executionCollector) visitEachChildWithUsageTarget(node *ast.Node, target *GraphSlice) bool {
	if node == nil {
		return false
	}
	previous := ec.usageTarget
	ec.usageTarget = target
	node.ForEachChild(ec.visitNodeVisitorConnectUsageTarget)
	ec.usageTarget = previous
	return false
}

func (ec *executionCollector) visitExpressionNode(node *ast.Expression, parentExpression *GraphSlice) *GraphSlice {
	rootExpr := parentExpression
	if parentExpression == nil && ast.IsExpressionNode(node) && !isInsideTypeOnlyHeritageExpression(node) {
		rootExpr = ec.buildValueNode(node)
	}
	ec.visitEachChildWithUsageTarget(node, rootExpr)
	if rootExpr != parentExpression {
		return rootExpr
	}
	return nil
}

func (ec *executionCollector) visitPipeCall(p *ParsedPipeCallResult, _ *ast.Node) *GraphSlice {
	s := ec.visitNode(p.subject)
	for i, pipedTransform := range p.args {
		// TODO: OOB argsouttype check
		callee, args := ec.extractCalleeAndArgs(pipedTransform)
		transformSlice := ec.buildSlice(ExecutionNode{
			kind:   ExecutionNodeKindTransform,
			node:   pipedTransform,
			typ:    p.argsOutType[i],
			callee: callee,
			args:   args,
		})
		*ec.parsed.Get(pipedTransform) = transformSlice
		ec.visitNodeAndConnectSlice(callee, transformSlice, ExecutionLinkKindTransformCallee)
		ec.visitNodesAndConnectSlice(args, transformSlice, ExecutionLinkKindTransformArg)
		s = ec.connectSlices(s, transformSlice, ExecutionLinkKindPipe)
	}
	return s
}

func (ec *executionCollector) visitEffectGenCall(p *EffectGenCallResult, node *ast.Node) *GraphSlice {
	s := ec.buildSlice(ExecutionNode{
		kind: ExecutionNodeKindLogicMerge,
		node: node,
		typ:  ec.tp.GetTypeAtLocation(node),
	})
	ast.ForEachReturnStatement(p.body, func(stmt *ast.Node) bool {
		if stmt.Kind == ast.KindReturnStatement {
			ec.visitNodeAndConnectSlice(stmt.Expression(), s, ExecutionLinkKindPotentialReturn)
		}
		return false
	})
	checker.ForEachYieldExpression(p.body, func(expr *ast.Node) bool {
		if expr != nil && expr.Expression() != nil {
			*ec.parsed.Get(expr) = nil
			ec.visitNodeAndConnectSlice(expr.Expression(), s, ExecutionLinkKindYieldable)
		}
		return false
	})
	if p.body != nil {
		ec.visitEachChildWithUsageTarget(p.body, s)
	}
	return s
}

func (ec *executionCollector) visitEffectFnCall(p *EffectFnCallResult, node *ast.Node) *GraphSlice {
	sExit := ec.buildSlice(ExecutionNode{
		kind: ExecutionNodeKindLogicMerge,
		node: p.functionNode,
		typ:  p.functionReturnType,
	})
	for i, pipedTransform := range p.pipeArguments {
		callee, args := ec.extractCalleeAndArgs(pipedTransform)
		transformSlice := ec.buildSlice(ExecutionNode{
			kind:   ExecutionNodeKindTransform,
			node:   pipedTransform,
			typ:    p.pipeArgsOutType[i], // TODO: OOB?
			callee: callee,
			args:   args,
		})
		*ec.parsed.Get(pipedTransform) = transformSlice
		ec.visitNodeAndConnectSlice(callee, transformSlice, ExecutionLinkKindTransformCallee)
		ec.visitNodesAndConnectSlice(args, transformSlice, ExecutionLinkKindTransformArg)
		sExit = ec.connectSlices(sExit, transformSlice, ExecutionLinkKindPipe)
	}
	if p.IsGenerator() {
		checker.ForEachYieldExpression(p.Body(), func(expr *ast.Node) bool {
			if expr != nil && expr.Expression() != nil {
				*ec.parsed.Get(expr) = nil
				ec.visitNodeAndConnectSlice(expr.Expression(), sExit, ExecutionLinkKindYieldable)
			}
			return false
		})
	}
	if ast.IsExpressionNode(p.Body()) {
		ec.visitNodeAndConnectSlice((p.Body()), sExit, ExecutionLinkKindPipe)
	} else {
		ast.ForEachReturnStatement(p.Body(), func(stmt *ast.Node) bool {
			if stmt.Kind == ast.KindReturnStatement {
				ec.visitNodeAndConnectSlice(stmt.Expression(), sExit, ExecutionLinkKindPotentialReturn)
			}
			return false
		})
		ec.visitEachChildWithUsageTarget(p.Body(), sExit)
	}
	// function with parameters
	s := ec.buildSlice(ExecutionNode{
		kind: ExecutionNodeKindFunction,
		node: node,
		typ:  ec.tp.GetTypeAtLocation(node),
	})
	for _, arg := range p.functionNode.Parameters() {
		ec.visitNodeAndConnectSlice(arg, s, ExecutionLinkKindParameter)
	}
	ec.connectSlices(sExit, s, ExecutionLinkKindPotentialReturn)
	return s
}

func (ec *executionCollector) visitSingleArgInlineCall(p *parsedSingleArgInlineCallTransform, node *ast.Node) *GraphSlice {
	s := ec.visitNode(p.Subject)
	callee, args := ec.extractCalleeAndArgs(p.Transform)
	transformSlice := ec.buildSlice(ExecutionNode{
		kind:   ExecutionNodeKindTransform,
		node:   p.Transform,
		typ:    ec.tp.GetTypeAtLocation(node),
		callee: callee,
		args:   args,
	})
	s = ec.connectSlices(s, transformSlice, ExecutionLinkKindPipe)
	ec.visitNodeAndConnectSlice(callee, transformSlice, ExecutionLinkKindTransformCallee)
	ec.visitNodesAndConnectSlice(args, transformSlice, ExecutionLinkKindTransformArg)
	return s
}

func (ec *executionCollector) visitDataFirstOrLastCall(p *ParsedDataFirstOrLastCall, node *ast.Node) *GraphSlice {
	s := ec.visitNode(p.subject)
	transformSlice := ec.buildSlice(ExecutionNode{
		kind: ExecutionNodeKindTransform,
		node: node,
		typ:  ec.tp.GetTypeAtLocation(node),
	})
	s = ec.connectSlices(s, transformSlice, ExecutionLinkKindPipe)
	ec.visitNodeAndConnectSlice(p.callee, s, ExecutionLinkKindTransformCallee)
	ec.visitNodesAndConnectSlice(p.args, s, ExecutionLinkKindTransformArg)
	return s
}

func (ec *executionCollector) visitFunctionLikeDeclaration(node *ast.Node) *GraphSlice {
	s := ec.buildSlice(ExecutionNode{
		kind: ExecutionNodeKindFunction,
		node: node,
		typ:  ec.tp.GetTypeAtLocation(node),
	})
	ec.visitNodesAndConnectSlice(node.Parameters(), s, ExecutionLinkKindParameter)
	fnBody := node.Body()
	if fnBody != nil {
		if ast.IsExpressionNode(fnBody) {
			ec.visitNodeAndConnectSlice(fnBody, s, ExecutionLinkKindPotentialReturn)
		} else {
			ast.ForEachReturnStatement(fnBody, func(stmt *ast.Node) bool {
				if stmt.Kind == ast.KindReturnStatement {
					ec.visitNodeAndConnectSlice(stmt.Expression(), s, ExecutionLinkKindPotentialReturn)
				}
				return false
			})
			ec.visitEachChildWithUsageTarget(fnBody, s)
		}
	}
	return s
}

func (tp *TypeParser) ExecutionFlow(sf *ast.SourceFile) *ExecutionFlow {
	if tp == nil || tp.checker == nil || sf == nil {
		return nil
	}

	return Cached(&tp.links.ExecutionFlow, sf, func() *ExecutionFlow {
		g := graph.New[ExecutionNode, ExecutionLink]()
		ec := &executionCollector{
			tp:     tp,
			g:      g,
			parsed: &core.LinkStore[*ast.Node, *GraphSlice]{},
		}
		ec.visitNode(sf.AsNode())
		return &ExecutionFlow{graph: g}
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
