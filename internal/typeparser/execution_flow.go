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
	tp                *TypeParser
	g                 *ExecutionFlow
	parsed            *core.LinkStore[*ast.Node, *GraphSlice]
	reportTransformTo *graph.NodeIndex
}

func (ec *executionCollector) buildValueNode(node *ast.Node) *GraphSlice {
	nodeIndex := ec.g.AddNode(ExecutionNode{
		Kind: ExecutionNodeKindValue,
		Node: node,
		Type: ec.tp.GetTypeAtLocation(node),
	})
	return &GraphSlice{
		Leading:  &nodeIndex,
		Trailing: &nodeIndex,
	}
}

func (ec *executionCollector) connectSlices(fromSlice *GraphSlice, toSlice *GraphSlice, kind ExecutionLinkKind) *GraphSlice {
	if fromSlice == nil {
		return toSlice
	}
	if toSlice == nil {
		return fromSlice
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
	// store parent ctx state
	previousReportTransform := ec.reportTransformTo
	if ec.reportTransformTo != nil {
		ec.reportTransformTo = nil
		if ast.IsParenthesizedExpression(node) {
			// keep as is
			ec.reportTransformTo = previousReportTransform
		} else if ast.IsCallExpression(node) {
			// Effect.as(true)
			ec.g.UpdateNode(*ec.reportTransformTo, func(en ExecutionNode) ExecutionNode {
				callExp := node.AsCallExpression()
				en.Callee = callExp.Expression
				en.Args = callExp.Arguments.Nodes
				return en
			})
		} else {
			// Effect.asVoid
			ec.g.UpdateNode(*ec.reportTransformTo, func(en ExecutionNode) ExecutionNode {
				en.Callee = node
				return en
			})
		}
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

	// restore back and exit
	ec.reportTransformTo = previousReportTransform
	return s
}

func (ec *executionCollector) visitNodeVisitor(node *ast.Node) bool {
	if node == nil {
		return false
	}
	ec.visitNode(node)
	return false
}

// same as visitNode, but enables reporting of the transform arg and node
func (ec *executionCollector) visitNodeCollectTransform(node *ast.Node, transformNode graph.NodeIndex) *GraphSlice {
	previousReportTransform := ec.reportTransformTo
	ec.reportTransformTo = &transformNode
	s := ec.visitNode(node)
	ec.reportTransformTo = previousReportTransform
	return s
}

func (ec *executionCollector) visitExpressionNode(node *ast.Expression) *GraphSlice {
	return ec.buildValueNode(node)
}

func (ec *executionCollector) visitPipeCall(p *ParsedPipeCallResult, node *ast.Node) *GraphSlice {
	s := ec.visitNode(p.Subject)
	for i, pipedTransform := range p.Args {
		// TODO: OOB argsouttype check
		transformNode := ec.g.AddNode(ExecutionNode{
			Kind: ExecutionNodeKindTransform,
			Node: pipedTransform,
			Type: p.ArgsOutType[i],
		})
		transformSlice := ec.visitNodeCollectTransform(pipedTransform, transformNode)
		s = ec.connectSlices(s, transformSlice, ExecutionLinkKindPipe)
	}
	node.ForEachChild(ec.visitNodeVisitor)
	return s
}

func (ec *executionCollector) visitEffectGenCall(p *EffectGenCallResult, node *ast.Node) *GraphSlice {
	genMerge := ec.g.AddNode(ExecutionNode{
		Kind: ExecutionNodeKindLogicMerge,
		Node: node,
		Type: ec.tp.GetTypeAtLocation(node),
	})
	s := &GraphSlice{
		Leading:  &genMerge,
		Trailing: &genMerge,
	}
	ast.ForEachReturnStatement(p.Body, func(stmt *ast.Node) bool {
		if stmt.Kind == ast.KindReturnStatement {
			returnNode := ec.visitNode(stmt.Expression())
			ec.connectSlices(returnNode, s, ExecutionLinkKindPotentialReturn)
		}
		return false
	})
	checker.ForEachYieldExpression(p.Body, func(expr *ast.Node) bool {
		if expr != nil && expr.Expression() != nil {
			yielded := ec.visitNode(expr.Expression())
			ec.connectSlices(yielded, s, ExecutionLinkKindYieldable)
		}
		return false
	})
	*ec.parsed.Get(p.GeneratorFunction.AsNode()) = nil // to prevent reparsing as function
	node.ForEachChild(ec.visitNodeVisitor)
	return s
}

func (ec *executionCollector) visitEffectFnCall(p *EffectFnCallResult, node *ast.Node) *GraphSlice {
	fnExit := ec.g.AddNode(ExecutionNode{
		Kind: ExecutionNodeKindLogicMerge,
		Node: p.FunctionNode,
		Type: p.FunctionReturnType,
	})
	sExit := &GraphSlice{
		Leading:  &fnExit,
		Trailing: &fnExit,
	}
	for i, pipedTransform := range p.PipeArguments {
		transformNode := ec.g.AddNode(ExecutionNode{
			Kind: ExecutionNodeKindTransform,
			Node: pipedTransform,
			Type: p.PipeArgsOutType[i], // TODO: OOB?
		})
		transformSlice := ec.visitNodeCollectTransform(pipedTransform, transformNode)
		sExit = ec.connectSlices(sExit, transformSlice, ExecutionLinkKindFnPipe)
	}
	if p.IsGenerator() {
		checker.ForEachYieldExpression(p.Body(), func(expr *ast.Node) bool {
			if expr != nil && expr.Expression() != nil {
				yielded := ec.visitNode(expr.Expression())
				ec.connectSlices(yielded, sExit, ExecutionLinkKindYieldable)
			}
			return false
		})
	}
	if ast.IsExpressionNode(p.Body()) {
		ec.connectSlices(ec.visitNode(p.Body()), sExit, ExecutionLinkKindPipe)
	} else {
		ast.ForEachReturnStatement(p.Body(), func(stmt *ast.Node) bool {
			if stmt.Kind == ast.KindReturnStatement {
				returnNode := ec.visitNode(stmt.Expression())
				ec.connectSlices(returnNode, sExit, ExecutionLinkKindPotentialReturn)
			}
			return false
		})
	}
	// function with parameters
	fnNode := ec.g.AddNode(ExecutionNode{
		Kind: ExecutionNodeKindFunction,
		Node: node,
		Type: ec.tp.GetTypeAtLocation(node),
	})
	s := &GraphSlice{
		Leading:  &fnNode,
		Trailing: &fnNode,
	}
	for _, arg := range p.FunctionNode.Arguments() {
		argNode := ec.visitNode(arg)
		ec.connectSlices(argNode, s, ExecutionLinkKindParameter)
	}
	ec.connectSlices(sExit, s, ExecutionLinkKindPotentialReturn)
	*ec.parsed.Get(p.FunctionNode.AsNode()) = nil // to prevent reparsing as function
	node.ForEachChild(ec.visitNodeVisitor)
	return s
}

func (ec *executionCollector) visitSingleArgInlineCall(p *parsedSingleArgInlineCallTransform, node *ast.Node) *GraphSlice {
	s := ec.visitNode(p.Subject)
	transformNode := ec.g.AddNode(ExecutionNode{
		Kind: ExecutionNodeKindTransform,
		Node: p.Transform,
		Type: ec.tp.GetTypeAtLocation(node),
	})
	transformSlice := ec.visitNodeCollectTransform(p.Transform, transformNode)
	s = ec.connectSlices(s, transformSlice, ExecutionLinkKindPipe)
	node.ForEachChild(ec.visitNodeVisitor)
	return s
}

func (ec *executionCollector) visitDataFirstOrLastCall(p *ParsedDataFirstOrLastCall, node *ast.Node) *GraphSlice {
	s := ec.visitNode(p.Subject)
	transformNode := ec.g.AddNode(ExecutionNode{
		Kind: ExecutionNodeKindTransform,
		Node: node,
		Type: ec.tp.GetTypeAtLocation(node),
	})
	transformSlice := &GraphSlice{
		Leading:  &transformNode,
		Trailing: &transformNode,
	}
	s = ec.connectSlices(s, transformSlice, ExecutionLinkKindPipe)
	// handle callee and args
	callee := ec.visitNode(p.Callee)
	ec.connectSlices(callee, s, ExecutionLinkKindTransformCallee)
	for _, arg := range p.Args {
		argNode := ec.visitNode(arg)
		ec.connectSlices(argNode, s, ExecutionLinkKindTransformArg)
	}
	node.ForEachChild(ec.visitNodeVisitor)
	return s
}

func (ec *executionCollector) visitFunctionLikeDeclaration(node *ast.Node) *GraphSlice {
	fnNode := ec.g.AddNode(ExecutionNode{
		Kind: ExecutionNodeKindFunction,
		Node: node,
		Type: ec.tp.GetTypeAtLocation(node),
	})
	s := &GraphSlice{
		Leading:  &fnNode,
		Trailing: &fnNode,
	}
	for _, arg := range node.Arguments() {
		argNode := ec.visitNode(arg)
		ec.connectSlices(argNode, s, ExecutionLinkKindParameter)
	}
	fnBody := node.Body()
	if ast.IsExpressionNode(fnBody) {
		returnNode := ec.visitNode(fnBody)
		ec.connectSlices(returnNode, s, ExecutionLinkKindPotentialReturn)
	} else {
		ast.ForEachReturnStatement(fnBody, func(stmt *ast.Node) bool {
			if stmt.Kind == ast.KindReturnStatement {
				returnNode := ec.visitNode(stmt.Expression())
				ec.connectSlices(returnNode, s, ExecutionLinkKindPotentialReturn)
			}
			return false
		})
	}
	node.ForEachChild(ec.visitNodeVisitor)
	return s
}

func (tp *TypeParser) ExecutionFlow(sf *ast.SourceFile) *ExecutionFlow {
	if tp == nil || tp.checker == nil || sf == nil {
		return nil
	}

	return Cached(&tp.links.ExecutionFlow, sf, func() *ExecutionFlow {
		g := graph.New[ExecutionNode, ExecutionLink]()
		ec := &executionCollector{
			tp: tp,
			g:  g,
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
