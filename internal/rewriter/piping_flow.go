package rewriter

import (
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

// PipingFlowTransformationReplacement describes a replacement operator in its
// normalized, pipeable form. A nil Arguments list represents a bare operator;
// a non-nil list represents a call, including an explicit zero-argument call.
type PipingFlowTransformationReplacement struct {
	Callee        *ast.Node
	TypeArguments *ast.NodeList
	Arguments     *ast.NodeList
}

// ReplacePipingFlowPrefix replaces the subject and the requested leading
// transformations with a complete replacement expression while retaining any
// transformations that follow the prefix.
func (t *Tracker) ReplacePipingFlowPrefix(
	sourceFile *ast.SourceFile,
	flow *typeparser.PipingFlow,
	transformationCount int,
	replacementSubject *ast.Node,
) bool {
	if t == nil || sourceFile == nil || flow == nil || replacementSubject == nil ||
		transformationCount <= 0 || transformationCount > len(flow.Transformations) {
		return false
	}

	last := &flow.Transformations[transformationCount-1]
	transformationNode := pipingFlowTransformationNode(last)
	var target *ast.Node
	switch last.Kind {
	case typeparser.TransformationKindCall,
		typeparser.TransformationKindDataFirst,
		typeparser.TransformationKindDataLast:
		target = transformationNode
	case typeparser.TransformationKindPipe:
		return t.replaceFunctionPipePrefix(sourceFile, transformationNode, replacementSubject)
	case typeparser.TransformationKindPipeable:
		return t.replaceMethodPipePrefix(sourceFile, transformationNode, replacementSubject)
	default:
		return false
	}
	if target == nil {
		return false
	}

	ast.SetParentInChildren(replacementSubject)
	t.ReplaceNode(sourceFile, target, replacementSubject, nil)
	return true
}

func (t *Tracker) replaceFunctionPipePrefix(sourceFile *ast.SourceFile, lastTransformation *ast.Node, replacementSubject *ast.Node) bool {
	callNode, call, argumentIndex := containingCallArgument(lastTransformation)
	if call == nil || argumentIndex < 1 {
		return false
	}

	remainingArguments := cloneNodeListNodesFrom(t, call.Arguments, argumentIndex+1)
	replacement := replacementSubject
	if len(remainingArguments) > 0 {
		arguments := append([]*ast.Node{replacementSubject}, remainingArguments...)
		replacement = t.NewCallExpression(
			t.DeepCloneNode(call.Expression),
			t.DeepCloneNode(call.QuestionDotToken),
			cloneNodeList(t, call.TypeArguments),
			t.NewNodeList(arguments),
			callNode.Flags,
		)
	}
	ast.SetParentInChildren(replacement)
	t.ReplaceNode(sourceFile, callNode, replacement, nil)
	return true
}

func (t *Tracker) replaceMethodPipePrefix(sourceFile *ast.SourceFile, lastTransformation *ast.Node, replacementSubject *ast.Node) bool {
	callNode, call, argumentIndex := containingCallArgument(lastTransformation)
	if call == nil || argumentIndex < 0 {
		return false
	}
	access := call.Expression.AsPropertyAccessExpression()
	if access == nil || access.Name() == nil {
		return false
	}

	remainingArguments := cloneNodeListNodesFrom(t, call.Arguments, argumentIndex+1)
	replacement := replacementSubject
	if len(remainingArguments) > 0 {
		replacementAccess := t.NewPropertyAccessExpression(
			replacementSubject,
			t.DeepCloneNode(access.QuestionDotToken),
			t.DeepCloneNode(access.Name()),
			call.Expression.Flags,
		)
		replacement = t.NewCallExpression(
			replacementAccess,
			t.DeepCloneNode(call.QuestionDotToken),
			cloneNodeList(t, call.TypeArguments),
			t.NewNodeList(remainingArguments),
			callNode.Flags,
		)
	}
	ast.SetParentInChildren(replacement)
	t.ReplaceNode(sourceFile, callNode, replacement, nil)
	return true
}

func containingCallArgument(node *ast.Node) (*ast.Node, *ast.CallExpression, int) {
	for child := node; child != nil && child.Parent != nil; child = child.Parent {
		parent := child.Parent
		if !ast.IsCallExpression(parent) {
			continue
		}
		call := parent.AsCallExpression()
		if call == nil || call.Arguments == nil {
			continue
		}
		for index, argument := range call.Arguments.Nodes {
			if argument == child {
				return parent, call, index
			}
		}
	}
	return nil, nil, -1
}

func pipingFlowTransformationNode(transformation *typeparser.PipingFlowTransformation) *ast.Node {
	if transformation == nil || transformation.Callee == nil {
		return nil
	}
	call := callExpressionForCallee(transformation.Callee)
	if transformation.Kind == typeparser.TransformationKindCall {
		// Curried factories decompose into a factory call and a trailing
		// application; the replaced node is the outermost applied call.
		call = appliedCallExpression(call)
	}
	if call != nil {
		return call.AsNode()
	}
	return transformation.Callee
}

func callExpressionForCallee(callee *ast.Node) *ast.CallExpression {
	if callee == nil || callee.Parent == nil || !ast.IsCallExpression(callee.Parent) {
		return nil
	}
	call := callee.Parent.AsCallExpression()
	if call == nil || call.Expression != callee {
		return nil
	}
	return call
}

// appliedCallExpression walks out of callee positions, returning the outermost
// call that applies the given call's result to its subject.
func appliedCallExpression(call *ast.CallExpression) *ast.CallExpression {
	if call == nil {
		return nil
	}
	current := call.AsNode()
	for current != nil && current.Parent != nil && ast.IsCallExpression(current.Parent) {
		parent := current.Parent.AsCallExpression()
		if parent == nil || parent.Expression != current {
			break
		}
		current = parent.AsNode()
	}
	return current.AsCallExpression()
}

// ReplacePipingFlowTransformation replaces a transformation while preserving
// how its input is applied in source: as a pipe argument, a data-first or
// data-last argument, or a unary/curried application.
func (t *Tracker) ReplacePipingFlowTransformation(
	sourceFile *ast.SourceFile,
	transformation *typeparser.PipingFlowTransformation,
	replacement PipingFlowTransformationReplacement,
) bool {
	if t == nil || sourceFile == nil || transformation == nil || replacement.Callee == nil {
		return false
	}
	transformationNode := pipingFlowTransformationNode(transformation)
	if transformationNode == nil {
		return false
	}

	switch transformation.Kind {
	case typeparser.TransformationKindPipe,
		typeparser.TransformationKindPipeable,
		typeparser.TransformationKindEffectFn,
		typeparser.TransformationKindEffectFnUntraced:
		replacementNode := t.pipingFlowOperator(replacement)
		ast.SetParentInChildren(replacementNode)
		t.ReplaceNode(sourceFile, transformationNode, replacementNode, nil)
		return true

	case typeparser.TransformationKindDataFirst, typeparser.TransformationKindDataLast:
		call := transformationNode.AsCallExpression()
		if call == nil || call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
			return false
		}
		arguments := cloneNodeListNodes(t, replacement.Arguments)
		subjectIndex := 0
		if transformation.Kind == typeparser.TransformationKindDataLast {
			subjectIndex = len(call.Arguments.Nodes) - 1
		}
		subject := t.DeepCloneNode(call.Arguments.Nodes[subjectIndex])
		if transformation.Kind == typeparser.TransformationKindDataFirst {
			arguments = append([]*ast.Node{subject}, arguments...)
		} else {
			arguments = append(arguments, subject)
		}
		replacementNode := t.NewCallExpression(
			replacement.Callee,
			nil,
			replacement.TypeArguments,
			t.NewNodeList(arguments),
			ast.NodeFlagsNone,
		)
		ast.SetParentInChildren(replacementNode)
		t.ReplaceNode(sourceFile, transformationNode, replacementNode, nil)
		return true

	case typeparser.TransformationKindCall:
		call := transformationNode.AsCallExpression()
		if call == nil || call.Arguments == nil || len(call.Arguments.Nodes) != 1 {
			return false
		}
		operator := t.pipingFlowOperator(replacement)
		replacementNode := t.NewCallExpression(
			operator,
			nil,
			nil,
			t.NewNodeList([]*ast.Node{t.DeepCloneNode(call.Arguments.Nodes[0])}),
			ast.NodeFlagsNone,
		)
		ast.SetParentInChildren(replacementNode)
		t.ReplaceNode(sourceFile, transformationNode, replacementNode, nil)
		return true
	}

	return false
}

func (t *Tracker) pipingFlowOperator(replacement PipingFlowTransformationReplacement) *ast.Node {
	if replacement.Arguments == nil && replacement.TypeArguments == nil {
		return replacement.Callee
	}
	return t.NewCallExpression(
		replacement.Callee,
		nil,
		replacement.TypeArguments,
		replacement.Arguments,
		ast.NodeFlagsNone,
	)
}

func cloneNodeListNodes(t *Tracker, list *ast.NodeList) []*ast.Node {
	if list == nil {
		return nil
	}
	nodes := make([]*ast.Node, len(list.Nodes))
	for i, node := range list.Nodes {
		nodes[i] = t.DeepCloneNode(node)
	}
	return nodes
}

func cloneNodeListNodesFrom(t *Tracker, list *ast.NodeList, start int) []*ast.Node {
	if list == nil || start >= len(list.Nodes) {
		return nil
	}
	nodes := make([]*ast.Node, len(list.Nodes)-start)
	for i, node := range list.Nodes[start:] {
		nodes[i] = t.DeepCloneNode(node)
	}
	return nodes
}

func cloneNodeList(t *Tracker, list *ast.NodeList) *ast.NodeList {
	if list == nil {
		return nil
	}
	return t.NewNodeList(cloneNodeListNodes(t, list))
}
