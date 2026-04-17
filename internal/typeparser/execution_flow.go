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

type executionCollector struct {
	tp     *TypeParser
	g      *ExecutionFlow
	parsed *core.LinkStore[*ast.Node, *GraphSlice]
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
		Kind: ExecutionNodeKindValue,
		Node: node,
		Type: ec.tp.GetTypeAtLocation(node),
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
		Kind: kind,
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
	} else if ast.IsExpressionNode(node) {
		s = ec.visitExpressionNode(node)
	} else {
		node.ForEachChild(ec.visitNodeVisitor)
	}
	// store to avoid double-traversal
	*ec.parsed.Get(node) = s

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

func (ec *executionCollector) visitNodeVisitor(node *ast.Node) bool {
	if node == nil {
		return false
	}
	ec.visitNode(node)
	return false
}

func (ec *executionCollector) visitExpressionNode(node *ast.Expression) *GraphSlice {
	return ec.buildValueNode(node)
}

func (ec *executionCollector) visitPipeCall(p *ParsedPipeCallResult, node *ast.Node) *GraphSlice {
	s := ec.visitNode(p.Subject)
	for i, pipedTransform := range p.Args {
		// TODO: OOB argsouttype check
		callee, args := ec.extractCalleeAndArgs(pipedTransform)
		transformSlice := ec.buildSlice(ExecutionNode{
			Kind:   ExecutionNodeKindTransform,
			Node:   pipedTransform,
			Type:   p.ArgsOutType[i],
			Callee: callee,
			Args:   args,
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
		Kind: ExecutionNodeKindLogicMerge,
		Node: node,
		Type: ec.tp.GetTypeAtLocation(node),
	})
	ast.ForEachReturnStatement(p.Body, func(stmt *ast.Node) bool {
		if stmt.Kind == ast.KindReturnStatement {
			ec.visitNodeAndConnectSlice(stmt.Expression(), s, ExecutionLinkKindPotentialReturn)
		}
		return false
	})
	checker.ForEachYieldExpression(p.Body, func(expr *ast.Node) bool {
		if expr != nil && expr.Expression() != nil {
			*ec.parsed.Get(expr) = nil
			ec.visitNodeAndConnectSlice(expr.Expression(), s, ExecutionLinkKindYieldable)
		}
		return false
	})
	if p.Body != nil {
		p.Body.ForEachChild(ec.visitNodeVisitor)
	}
	return s
}

func (ec *executionCollector) visitEffectFnCall(p *EffectFnCallResult, node *ast.Node) *GraphSlice {
	sExit := ec.buildSlice(ExecutionNode{
		Kind: ExecutionNodeKindLogicMerge,
		Node: p.FunctionNode,
		Type: p.FunctionReturnType,
	})
	for i, pipedTransform := range p.PipeArguments {
		callee, args := ec.extractCalleeAndArgs(pipedTransform)
		transformSlice := ec.buildSlice(ExecutionNode{
			Kind:   ExecutionNodeKindTransform,
			Node:   pipedTransform,
			Type:   p.PipeArgsOutType[i], // TODO: OOB?
			Callee: callee,
			Args:   args,
		})
		*ec.parsed.Get(pipedTransform) = transformSlice
		ec.visitNodeAndConnectSlice(callee, transformSlice, ExecutionLinkKindTransformCallee)
		ec.visitNodesAndConnectSlice(args, transformSlice, ExecutionLinkKindTransformArg)
		sExit = ec.connectSlices(sExit, transformSlice, ExecutionLinkKindFnPipe)
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
		p.Body().ForEachChild(ec.visitNodeVisitor)
	}
	// function with parameters
	s := ec.buildSlice(ExecutionNode{
		Kind: ExecutionNodeKindFunction,
		Node: node,
		Type: ec.tp.GetTypeAtLocation(node),
	})
	for _, arg := range p.FunctionNode.Parameters() {
		ec.visitNodeAndConnectSlice(arg, s, ExecutionLinkKindParameter)
	}
	ec.connectSlices(sExit, s, ExecutionLinkKindPotentialReturn)
	return s
}

func (ec *executionCollector) visitSingleArgInlineCall(p *parsedSingleArgInlineCallTransform, node *ast.Node) *GraphSlice {
	s := ec.visitNode(p.Subject)
	callee, args := ec.extractCalleeAndArgs(p.Transform)
	transformSlice := ec.buildSlice(ExecutionNode{
		Kind:   ExecutionNodeKindTransform,
		Node:   p.Transform,
		Type:   ec.tp.GetTypeAtLocation(node),
		Callee: callee,
		Args:   args,
	})
	s = ec.connectSlices(s, transformSlice, ExecutionLinkKindPipe)
	ec.visitNodeAndConnectSlice(callee, transformSlice, ExecutionLinkKindTransformCallee)
	ec.visitNodesAndConnectSlice(args, transformSlice, ExecutionLinkKindTransformArg)
	return s
}

func (ec *executionCollector) visitDataFirstOrLastCall(p *ParsedDataFirstOrLastCall, node *ast.Node) *GraphSlice {
	s := ec.visitNode(p.Subject)
	transformSlice := ec.buildSlice(ExecutionNode{
		Kind: ExecutionNodeKindTransform,
		Node: node,
		Type: ec.tp.GetTypeAtLocation(node),
	})
	s = ec.connectSlices(s, transformSlice, ExecutionLinkKindPipe)
	ec.visitNodeAndConnectSlice(p.Callee, s, ExecutionLinkKindTransformCallee)
	ec.visitNodesAndConnectSlice(p.Args, s, ExecutionLinkKindTransformArg)
	return s
}

func (ec *executionCollector) visitFunctionLikeDeclaration(node *ast.Node) *GraphSlice {
	s := ec.buildSlice(ExecutionNode{
		Kind: ExecutionNodeKindFunction,
		Node: node,
		Type: ec.tp.GetTypeAtLocation(node),
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
			fnBody.ForEachChild(ec.visitNodeVisitor)
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
