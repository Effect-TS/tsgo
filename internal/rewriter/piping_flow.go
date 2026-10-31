package rewriter

import (
	"slices"
	"strings"

	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/astnav"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
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
// transformations that follow the prefix. It edits nothing and returns false
// when the prefix ends in an Effect.fn step, whose subject is a function body
// rather than an expression.
func (t *Tracker) ReplacePipingFlowPrefix(
	sourceFile *ast.SourceFile,
	flow *typeparser.PipingFlow,
	transformationCount int,
	replacementSubject *ast.Node,
) bool {
	if t == nil || sourceFile == nil || flow == nil || replacementSubject == nil ||
		transformationCount <= 0 || transformationCount > flow.Transformations().Len() {
		return false
	}

	last := flow.Transformations().At(transformationCount - 1)
	transformationNode := pipingFlowTransformationNode(last)
	var target *ast.Node
	switch last.Kind() {
	case typeparser.TransformationKindCall,
		typeparser.TransformationKindDataFirst,
		typeparser.TransformationKindDataLast,
		typeparser.TransformationKindCallWithOptionals:
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

func pipingFlowTransformationNode(transformation typeparser.PipingFlowTransformation) *ast.Node {
	if transformation.Callee() == nil {
		return nil
	}
	call := callExpressionForCallee(transformation.Callee())
	if transformation.Kind() == typeparser.TransformationKindCall && transformation.Args().Len() > 0 {
		// Curried factories decompose into a factory call and a trailing
		// application; the replaced node is the applied call.
		call = typeparser.CurriedApplicationCall(call)
	}
	if call != nil {
		return call.AsNode()
	}
	return transformation.Callee()
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

// RemoveTrailingPipingFlowTransformation removes the last step while retaining
// its input and any earlier transformations. Unsupported flows produce no edits.
func (t *Tracker) RemoveTrailingPipingFlowTransformation(sf *ast.SourceFile, flow *typeparser.PipingFlow) bool {
	if t == nil || sf == nil || flow == nil || flow.Transformations().Len() == 0 {
		return false
	}
	last := flow.Transformations().Len() - 1
	terminal := flow.Transformations().At(last)
	if terminal.Callee() == nil || terminal.Callee().Flags&ast.NodeFlagsOptionalChain != 0 {
		return false
	}
	switch terminal.Kind() {
	case typeparser.TransformationKindDataFirst, typeparser.TransformationKindDataLast, typeparser.TransformationKindCall:
		call := callExpressionForCallee(terminal.Callee())
		if call == nil || call.AsNode().Flags&ast.NodeFlagsOptionalChain != 0 {
			return false
		}
		callNode := call.AsNode()
		if terminal.Kind() == typeparser.TransformationKindCall {
			for callNode != flow.Node() {
				applied := callNode
				for applied.Parent != nil && applied.Parent.Kind == ast.KindParenthesizedExpression {
					applied = applied.Parent
				}
				if applied.Parent == nil || !ast.IsCallExpression(applied.Parent) || applied.Parent.AsCallExpression().Expression != applied {
					break
				}
				callNode = applied.Parent
				if callNode.Flags&ast.NodeFlagsOptionalChain != 0 {
					return false
				}
			}
		}
		input := flow.TransformationInputNode(last)
		if terminal.Kind() == typeparser.TransformationKindCall {
			call = callNode.AsCallExpression()
			if call.Arguments == nil || len(call.Arguments.Nodes) != 1 {
				return false
			}
			input = call.Arguments.Nodes[0]
		}
		if input == nil {
			return false
		}
		t.ReplaceNode(sf, callNode, input, nil)
		return true

	case typeparser.TransformationKindPipe, typeparser.TransformationKindPipeable:
		node := pipingFlowTransformationNode(terminal)
		if node == nil || node.Flags&ast.NodeFlagsOptionalChain != 0 {
			return false
		}
		callNode, call, index := containingCallArgument(node)
		if call == nil || index < 0 || callNode.Flags&ast.NodeFlagsOptionalChain != 0 {
			return false
		}
		arguments := call.Arguments.Nodes
		minimum := 1
		if terminal.Kind() == typeparser.TransformationKindPipe {
			minimum = 2
		}
		if len(arguments) == minimum {
			var input *ast.Node
			if terminal.Kind() == typeparser.TransformationKindPipe {
				input = arguments[0]
			} else if ast.IsPropertyAccessExpression(call.Expression) {
				input = call.Expression.AsPropertyAccessExpression().Expression
			}
			if input == nil {
				return false
			}
			t.ReplaceNode(sf, callNode, input, nil)
			return true
		}
		var span core.TextRange
		switch {
		case index < len(arguments)-1:
			span = core.NewTextRange(scanner.GetTokenPosOfNode(arguments[index], sf, false), scanner.GetTokenPosOfNode(arguments[index+1], sf, false))
		case index > 0:
			span = core.NewTextRange(arguments[index-1].End(), arguments[index].End())
			start := scanner.GetTokenPosOfNode(arguments[index], sf, false)
			text := sf.Text()
			trivia := text[span.Pos():start]
			if strings.Contains(trivia, "//") || strings.Contains(trivia, "/*") {
				separator := scanner.SkipTrivia(text, span.Pos())
				if separator >= start || text[separator] != ',' {
					return false
				}
				trimStart := start
				for trimStart > span.Pos() && (trimStart-1 == separator || text[trimStart-1] == ' ' || text[trimStart-1] == '\t') {
					trimStart--
				}
				t.DeleteRange(sf, core.NewTextRange(trimStart, span.End()))
				if separator < trimStart {
					t.DeleteRange(sf, core.NewTextRange(separator, separator+1))
				}
				return true
			}
		default:
			return false
		}
		t.DeleteRange(sf, span)
		return true
	}
	return false
}

// ReplacePipingFlowTransformation replaces a transformation while preserving
// how its input is applied in source: as a pipe argument, a data-first or
// data-last argument, or a unary/curried application. A call with optional
// arguments keeps them after the input.
func (t *Tracker) ReplacePipingFlowTransformation(
	sourceFile *ast.SourceFile,
	transformation *typeparser.PipingFlowTransformation,
	replacement PipingFlowTransformationReplacement,
) bool {
	if t == nil || sourceFile == nil || transformation == nil || replacement.Callee == nil {
		return false
	}
	transformationNode := pipingFlowTransformationNode(*transformation)
	if transformationNode == nil {
		return false
	}

	switch transformation.Kind() {
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
		if transformation.Kind() == typeparser.TransformationKindDataLast {
			subjectIndex = len(call.Arguments.Nodes) - 1
		}
		subject := t.DeepCloneNode(call.Arguments.Nodes[subjectIndex])
		if transformation.Kind() == typeparser.TransformationKindDataFirst {
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

	case typeparser.TransformationKindCallWithOptionals:
		call := transformationNode.AsCallExpression()
		if call == nil || call.Arguments == nil || len(call.Arguments.Nodes) < 2 {
			return false
		}
		replacementNode := t.NewCallExpression(
			t.pipingFlowOperator(replacement),
			nil,
			nil,
			cloneNodeList(t, call.Arguments),
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

// CollapsePipingFlowTransformations replaces an inclusive range of adjacent
// steps with one operator at the last step. Nested applications are rewritten
// together so removing an input step cannot overlap the replacement edit.
func (t *Tracker) CollapsePipingFlowTransformations(sourceFile *ast.SourceFile, flow *typeparser.PipingFlow, start, end int, replacement PipingFlowTransformationReplacement) bool {
	if t == nil || sourceFile == nil || flow == nil || start < 0 || end < start || end >= flow.Transformations().Len() || replacement.Callee == nil {
		return false
	}
	steps := make(map[*ast.Node]int)
	emptiedPipes := make(map[*ast.Node]typeparser.TransformationKind)
	for i := start; i <= end; i++ {
		node := pipingFlowTransformationNode(flow.Transformations().At(i))
		if node == nil {
			return false
		}
		steps[node] = i
		kind := flow.Transformations().At(i).Kind()
		if i < end && (kind == typeparser.TransformationKindPipe || kind == typeparser.TransformationKindPipeable) {
			call, _, _ := containingCallArgument(node)
			emptiedPipes[call] = kind
		}
	}
	// Find the smallest source subtree containing all edited steps. Pipe
	// arguments require their containing call so they can be removed as a list.
	target := pipingFlowTransformationNode(flow.Transformations().At(end))
	for i := start; i <= end; i++ {
		node := pipingFlowTransformationNode(flow.Transformations().At(i))
		if isPipingFlowArgument(flow.Transformations().At(i).Kind()) {
			call, _, _ := containingCallArgument(node)
			node = call
		}
		if node == nil {
			return false
		}
		for target != nil && !nodeWithin(node, target) {
			target = target.Parent
		}
	}
	if target == nil {
		return false
	}
	var replaced *ast.Node
	var visitor *ast.NodeVisitor
	visitor = ast.NewNodeVisitor(func(node *ast.Node) *ast.Node {
		index, matched := steps[node]
		if !matched {
			updated := visitor.VisitEachChild(node)
			if kind, changed := emptiedPipes[node]; changed {
				call := updated.AsCallExpression()
				if kind == typeparser.TransformationKindPipeable && (call.Arguments == nil || len(call.Arguments.Nodes) == 0) {
					return call.Expression.AsPropertyAccessExpression().Expression
				}
				if kind == typeparser.TransformationKindPipe && call.Arguments != nil && len(call.Arguments.Nodes) == 1 {
					return call.Arguments.Nodes[0]
				}
			}
			return updated
		}
		step := flow.Transformations().At(index)
		if isPipingFlowArgument(step.Kind()) {
			if index < end {
				return nil
			}
			replaced = t.pipingFlowOperator(replacement)
			return replaced
		}
		input := flow.TransformationInputNode(index)
		if input == nil {
			return node
		}
		subject := visitor.VisitNode(input)
		if index < end {
			return subject
		}
		arguments := cloneNodeListNodes(t, replacement.Arguments)
		switch step.Kind() {
		case typeparser.TransformationKindDataFirst:
			arguments = append([]*ast.Node{subject}, arguments...)
		case typeparser.TransformationKindDataLast:
			arguments = append(arguments, subject)
		default:
			replaced = t.NewCallExpression(t.pipingFlowOperator(replacement), nil, nil, t.NewNodeList([]*ast.Node{subject}), ast.NodeFlagsNone)
			return replaced
		}
		replaced = t.NewCallExpression(replacement.Callee, nil, replacement.TypeArguments, t.NewNodeList(arguments), ast.NodeFlagsNone)
		return replaced
	}, t.NodeFactory, ast.NodeVisitorHooks{})
	visited := visitor.VisitNode(target)
	result := t.DeepCloneNode(visited)
	removed := make([]*ast.Node, 0, end-start+1)
	for i := start; i <= end; i++ {
		removed = append(removed, pipingFlowTransformationNode(flow.Transformations().At(i)))
	}
	t.retainPipingFlowComments(sourceFile, target, visited, result, removed, flow.TransformationInputNode(start), replaced)
	ast.SetParentInChildren(result)
	t.ReplaceNode(sourceFile, target, result, nil)
	return true
}

// retainPipingFlowComments re-attaches source comments to the synthesized
// clone of a collapsed flow. Comments inside the removed steps move to the
// replacement; every other comment stays with the innermost kept node that
// contains it. Comments outside target stay in the source text, which the
// replacement leaves untouched.
func (t *Tracker) retainPipingFlowComments(sourceFile *ast.SourceFile, target, visited, result *ast.Node, removed []*ast.Node, kept, replaced *ast.Node) {
	comments := sourceComments(sourceFile, scanner.GetTokenPosOfNode(target, sourceFile, false), target.End())
	if len(comments) == 0 {
		return
	}
	children := func(node *ast.Node) []*ast.Node {
		var nodes []*ast.Node
		node.ForEachChild(func(child *ast.Node) bool {
			nodes = append(nodes, child)
			return false
		})
		return nodes
	}
	// Nodes built for the replacement have no source position; updated and
	// untouched nodes keep theirs, so they own the comments within their range.
	clones := make(map[*ast.Node]*ast.Node)
	replacedClone := result
	var pair func(original, clone *ast.Node)
	pair = func(original, clone *ast.Node) {
		if original == replaced {
			replacedClone = clone
		}
		if original.Pos() >= 0 {
			clones[original] = clone
		}
		originals, cloned := children(original), children(clone)
		if len(originals) == len(cloned) {
			for i := range originals {
				pair(originals[i], cloned[i])
			}
		}
	}
	pair(visited, result)
	text := sourceFile.Text()
	// The kept subject keeps its leading trivia, and a removed step takes its
	// text plus the trivia up to the next token.
	inKept := func(comment ast.CommentRange) bool {
		return kept != nil && kept.Pos() >= 0 && comment.Pos() >= kept.Pos() && comment.End() <= kept.End()
	}
	inRemoved := func(comment ast.CommentRange) bool {
		return slices.ContainsFunc(removed, func(node *ast.Node) bool {
			return node != nil && node.Pos() >= 0 && comment.Pos() >= node.Pos() && comment.End() <= scanner.SkipTrivia(text, node.End())
		})
	}
	// A kept node owns the text from its first token on. Its leading trivia
	// belongs before it in its parent: a comment there must not end up between
	// the node's first token and the rest, as in "return // comment\n value".
	inside := func(comment ast.CommentRange, node *ast.Node) bool {
		return node.Pos() >= 0 && comment.Pos() >= scanner.GetTokenPosOfNode(node, sourceFile, false) && comment.End() <= node.End()
	}
	for _, comment := range comments {
		if !inKept(comment) && inRemoved(comment) {
			t.addPipingFlowComment(text, replacedClone, comment, true)
			continue
		}
		var owner *ast.Node
		for original := range clones {
			if inside(comment, original) && (owner == nil || original.End()-original.Pos() < owner.End()-owner.Pos()) {
				owner = original
			}
		}
		if owner == nil {
			owner = visited
		}
		// Attach to the nearest kept child so the comment keeps its place; an
		// owner without kept children around the comment takes it itself.
		var before, after *ast.Node
		for _, child := range children(owner) {
			if clones[child] == nil {
				continue
			}
			if child.End() <= comment.Pos() {
				before = child
			} else if after == nil && scanner.GetTokenPosOfNode(child, sourceFile, false) >= comment.End() {
				after = child
			}
		}
		switch {
		case after != nil:
			t.addPipingFlowComment(text, clones[after], comment, true)
		case before != nil:
			t.addPipingFlowComment(text, clones[before], comment, false)
		case clones[owner] == nil:
			t.addPipingFlowComment(text, result, comment, true)
		case clones[owner].Kind == ast.KindBlock:
			// Function bodies skip the comments of their block node, so keep
			// the comment inside the braces on a placeholder statement.
			block := clones[owner].AsBlock()
			placeholder := t.NodeFactory.NewNotEmittedStatement()
			t.addPipingFlowComment(text, placeholder, comment, true)
			block.Statements = t.NewNodeList(append(slices.Clone(block.Statements.Nodes), placeholder))
		default:
			t.addPipingFlowComment(text, clones[owner], comment, true)
		}
	}
}

// sourceComments lists the comments between start and end in source order.
// Tokens are skipped by their parsed extent, so comment-like text inside
// literals, templates and regular expressions is not reported.
func sourceComments(sourceFile *ast.SourceFile, start, end int) []ast.CommentRange {
	text := sourceFile.Text()
	s := scanner.NewScanner()
	s.SetSkipTrivia(false)
	s.SetText(text[:end])
	s.ResetPos(start)
	var comments []ast.CommentRange
	for token := s.Scan(); token != ast.KindEndOfFile; token = s.Scan() {
		switch token {
		case ast.KindSingleLineCommentTrivia, ast.KindMultiLineCommentTrivia:
			rest := strings.TrimLeft(text[s.TokenEnd():], " \t")
			newLine := strings.HasPrefix(rest, "\n") || strings.HasPrefix(rest, "\r")
			comments = append(comments, ast.CommentRange{TextRange: core.NewTextRange(s.TokenStart(), s.TokenEnd()), Kind: token, HasTrailingNewLine: newLine})
		case ast.KindWhitespaceTrivia, ast.KindNewLineTrivia, ast.KindConflictMarkerTrivia:
		default:
			if original := astnav.GetTokenAtPosition(sourceFile, s.TokenStart()); original != nil && original.End() > s.TokenEnd() {
				s.ResetPos(min(original.End(), end))
			}
		}
	}
	return comments
}

func (t *Tracker) addPipingFlowComment(text string, node *ast.Node, comment ast.CommentRange, leading bool) {
	body := text[comment.Pos()+2 : comment.End()]
	// A line comment always needs its line break, or it would swallow the code after it.
	newLine := true
	if comment.Kind == ast.KindMultiLineCommentTrivia {
		body = strings.TrimSuffix(body, "*/")
		newLine = comment.HasTrailingNewLine
	}
	if leading {
		t.AddSyntheticLeadingComment(node, comment.Kind, body, newLine)
	} else {
		t.AddSyntheticTrailingComment(node, comment.Kind, body, newLine)
	}
}

func isPipingFlowArgument(kind typeparser.TransformationKind) bool {
	switch kind {
	case typeparser.TransformationKindPipe, typeparser.TransformationKindPipeable, typeparser.TransformationKindEffectFn, typeparser.TransformationKindEffectFnUntraced:
		return true
	}
	return false
}

func nodeWithin(node, ancestor *ast.Node) bool {
	for node != nil {
		if node == ancestor {
			return true
		}
		node = node.Parent
	}
	return false
}
