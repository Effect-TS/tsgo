// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"sort"

	"github.com/effect-ts/tsgo/internal/readonly"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// TransformationKind represents how a transformation was expressed in source code.
type TransformationKind string

const (
	TransformationKindPipe      TransformationKind = "pipe"
	TransformationKindPipeable  TransformationKind = "pipeable"
	TransformationKindDataFirst TransformationKind = "dataFirst"
	TransformationKindDataLast  TransformationKind = "dataLast"
	TransformationKindCall      TransformationKind = "call"
	// TransformationKindCallWithOptionals is callee(subject, args...) where the
	// callee has a single call signature whose only required parameter is the
	// subject. Unlike dataFirst, it has no pipeable counterpart.
	TransformationKindCallWithOptionals TransformationKind = "callWithOptionals"
	TransformationKindEffectFn          TransformationKind = "effectFn"
	TransformationKindEffectFnUntraced  TransformationKind = "effectFnUntraced"
)

// PipingFlowTransformation represents a single transformation step in a piping flow.
type PipingFlowTransformation struct {
	kind          TransformationKind   // How the transformation was expressed
	callee        *ast.Node            // The function being applied (e.g., Effect.map)
	typeArguments *ast.NodeList        // Explicit type arguments to the transformation call, if any
	args          []*ast.Node          // Arguments to the transformation, or nil for constants/single-arg calls
	outType       *lazy[*checker.Type] // The resulting type after this transformation (may be nil)
}

func (r PipingFlowTransformation) Kind() TransformationKind { return r.kind }

func (r PipingFlowTransformation) Callee() *ast.Node { return r.callee }

func (r PipingFlowTransformation) TypeArguments() *ast.NodeList { return r.typeArguments }

func (r PipingFlowTransformation) Args() readonly.Slice[*ast.Node] { return readonly.New(r.args) }

func (r PipingFlowTransformation) OutType() *checker.Type { return r.outType.get() }

// PipingFlowSubject is the starting expression of a piping flow.
type PipingFlowSubject struct {
	node    *ast.Node            // The expression node
	outType *lazy[*checker.Type] // The type of the subject expression (may be nil)
}

func (r PipingFlowSubject) Node() *ast.Node { return r.node }

func (r PipingFlowSubject) OutType() *checker.Type { return r.outType.get() }

// PartialPipingFlow represents a logical piping flow that may not correspond to
// a complete source expression, such as a prefix of a pipe call.
type PartialPipingFlow struct {
	subject         PipingFlowSubject          // The starting expression and its type
	transformations []PipingFlowTransformation // Ordered list of transformations
}

func (r PartialPipingFlow) Subject() PipingFlowSubject { return r.subject }

func (r PartialPipingFlow) Transformations() readonly.Slice[PipingFlowTransformation] {
	return readonly.New(r.transformations)
}

// MatchesPrefix checks the subject and leading transformations, allowing extra
// steps afterward. With no transformation predicates it checks only the subject.
// A nil flow returns false. Predicates receive local copies.
func (flow *PartialPipingFlow) MatchesPrefix(subject func(PipingFlowSubject) bool, transformations ...func(PipingFlowTransformation) bool) bool {
	return flow.matchesShape(false, subject, transformations)
}

// MatchesExactly checks the subject and all transformations. The number of
// transformations must equal the number of transformation predicates.
// A nil flow returns false. Predicates receive local copies.
func (flow *PartialPipingFlow) MatchesExactly(subject func(PipingFlowSubject) bool, transformations ...func(PipingFlowTransformation) bool) bool {
	return flow.matchesShape(true, subject, transformations)
}

func (flow *PartialPipingFlow) matchesShape(exact bool, subject func(PipingFlowSubject) bool, transformations []func(PipingFlowTransformation) bool) bool {
	if flow == nil || len(flow.transformations) < len(transformations) ||
		exact && len(flow.transformations) != len(transformations) {
		return false
	}
	if !subject(flow.subject) {
		return false
	}
	for i, predicate := range transformations {
		if !predicate(flow.transformations[i]) {
			return false
		}
	}
	return true
}

// TransformationSequenceMatch identifies consecutive steps in a piping flow.
// Transformations is a read-only view into the original flow's slice.
type TransformationSequenceMatch struct {
	start           int
	transformations []PipingFlowTransformation
}

func (r TransformationSequenceMatch) Start() int { return r.start }

func (r TransformationSequenceMatch) Transformations() readonly.Slice[PipingFlowTransformation] {
	return readonly.New(r.transformations)
}

// FindTransformationSequences returns all consecutive matches in source order,
// including overlaps. The view is nil for a nil flow, no predicates, or no match.
// Predicates receive local copies and must not depend on earlier calls.
func (flow *PartialPipingFlow) FindTransformationSequences(predicates ...func(PipingFlowTransformation) bool) readonly.Slice[TransformationSequenceMatch] {
	if flow == nil || len(predicates) == 0 {
		return readonly.Slice[TransformationSequenceMatch]{}
	}
	var matches []TransformationSequenceMatch
	for start := 0; start <= len(flow.transformations)-len(predicates); start++ {
		matched := true
		for offset, predicate := range predicates {
			if !predicate(flow.transformations[start+offset]) {
				matched = false
				break
			}
		}
		if matched {
			matches = append(matches, TransformationSequenceMatch{
				start:           start,
				transformations: flow.transformations[start : start+len(predicates)],
			})
		}
	}
	return readonly.New(matches)
}

// CopyPrefix returns a copy containing the first transformationCount
// transformations. It returns nil when transformationCount is out of bounds.
func (flow *PartialPipingFlow) CopyPrefix(transformationCount int) *PartialPipingFlow {
	if flow == nil || transformationCount < 0 || transformationCount > len(flow.transformations) {
		return nil
	}
	transformations := make([]PipingFlowTransformation, transformationCount)
	copy(transformations, flow.transformations[:transformationCount])
	return &PartialPipingFlow{
		subject:         flow.subject,
		transformations: transformations,
	}
}

// TransformationInputType returns the type immediately before the indexed step.
// Missing types and indices outside the flow return nil.
func (flow *PartialPipingFlow) TransformationInputType(index int) *checker.Type {
	if flow == nil || index < 0 || index >= len(flow.transformations) {
		return nil
	}
	if index == 0 {
		return flow.subject.OutType()
	}
	return flow.transformations[index-1].OutType()
}

// TransformationInputNode returns the standalone source expression that feeds
// the transformation at index, when that expression is represented directly
// in the syntax tree.
func (flow *PartialPipingFlow) TransformationInputNode(index int) *ast.Node {
	if flow == nil || index < 0 || index >= len(flow.transformations) {
		return nil
	}
	transformation := &flow.transformations[index]
	call := pipingFlowTransformationCall(transformation)
	if transformation.kind == TransformationKindCall {
		// Curried factories decompose into a factory call and a trailing
		// application; the input feeds the outermost applied call.
		call = appliedCallOf(call)
	}
	if call != nil && call.Arguments != nil {
		switch transformation.kind {
		case TransformationKindCall:
			if len(call.Arguments.Nodes) == 1 {
				return call.Arguments.Nodes[0]
			}
		case TransformationKindDataFirst, TransformationKindCallWithOptionals:
			if len(call.Arguments.Nodes) > 0 {
				return call.Arguments.Nodes[0]
			}
		case TransformationKindDataLast:
			if len(call.Arguments.Nodes) > 0 {
				return call.Arguments.Nodes[len(call.Arguments.Nodes)-1]
			}
		}
	}
	if index == 0 {
		return flow.subject.node
	}
	return nil
}

// appliedCallOf walks out of callee positions, returning the outermost call
// that applies the given call's result to its subject.
func appliedCallOf(call *ast.CallExpression) *ast.CallExpression {
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
	if current == nil {
		return nil
	}
	return current.AsCallExpression()
}

func pipingFlowTransformationCall(transformation *PipingFlowTransformation) *ast.CallExpression {
	if transformation == nil || transformation.callee == nil || transformation.callee.Parent == nil ||
		!ast.IsCallExpression(transformation.callee.Parent) {
		return nil
	}
	call := transformation.callee.Parent.AsCallExpression()
	if call == nil || call.Expression != transformation.callee {
		return nil
	}
	return call
}

// PipingFlow represents a complete piping flow rooted at a source expression.
type PipingFlow struct {
	node    *ast.Node // The outermost expression encompassing the entire flow
	partial PartialPipingFlow
}

func (r PipingFlow) Node() *ast.Node { return r.node }

func (r PipingFlow) Partial() PartialPipingFlow { return r.partial }

func (r PipingFlow) Subject() PipingFlowSubject { return r.partial.subject }

func (r PipingFlow) Transformations() readonly.Slice[PipingFlowTransformation] {
	return readonly.New(r.partial.transformations)
}

// MatchesPrefix checks the subject and leading transformations, allowing extra steps.
// It is defined explicitly to support a nil *PipingFlow.
func (flow *PipingFlow) MatchesPrefix(subject func(PipingFlowSubject) bool, transformations ...func(PipingFlowTransformation) bool) bool {
	if flow == nil {
		return false
	}
	return flow.partial.MatchesPrefix(subject, transformations...)
}

// MatchesExactly checks the subject and all transformations, requiring an exact length.
// It is defined explicitly to support a nil *PipingFlow.
func (flow *PipingFlow) MatchesExactly(subject func(PipingFlowSubject) bool, transformations ...func(PipingFlowTransformation) bool) bool {
	if flow == nil {
		return false
	}
	return flow.partial.MatchesExactly(subject, transformations...)
}

// FindTransformationSequences returns all consecutive matches, including overlaps.
// It is defined explicitly to support a nil *PipingFlow.
func (flow *PipingFlow) FindTransformationSequences(predicates ...func(PipingFlowTransformation) bool) readonly.Slice[TransformationSequenceMatch] {
	if flow == nil {
		return readonly.Slice[TransformationSequenceMatch]{}
	}
	return flow.partial.FindTransformationSequences(predicates...)
}

// CopyPrefix returns a partial copy containing the first transformationCount
// transformations. It is defined explicitly to support a nil *PipingFlow.
func (flow *PipingFlow) CopyPrefix(transformationCount int) *PartialPipingFlow {
	if flow == nil {
		return nil
	}
	return flow.partial.CopyPrefix(transformationCount)
}

// TransformationInputType returns the type immediately before the indexed step.
func (flow *PipingFlow) TransformationInputType(index int) *checker.Type {
	if flow == nil {
		return nil
	}
	return flow.partial.TransformationInputType(index)
}

// TransformationInputNode returns the source expression that feeds the
// transformation at index.
func (flow *PipingFlow) TransformationInputNode(index int) *ast.Node {
	if flow == nil {
		return nil
	}
	return flow.partial.TransformationInputNode(index)
}

// ParsedPipeCallResult is the result of parsing a pipe or pipeable call.
type ParsedPipeCallResult struct {
	node        *ast.CallExpression
	subject     *ast.Node
	args        []*ast.Node
	kind        TransformationKind
	subjectType *lazy[*checker.Type]
	argsOutType *lazy[[]*checker.Type]
}

func (r ParsedPipeCallResult) Node() *ast.CallExpression { return r.node }

func (r ParsedPipeCallResult) Subject() *ast.Node { return r.subject }

func (r ParsedPipeCallResult) Args() readonly.Slice[*ast.Node] { return readonly.New(r.args) }

func (r ParsedPipeCallResult) Kind() TransformationKind { return r.kind }

func (r ParsedPipeCallResult) SubjectType() *checker.Type { return r.subjectType.get() }

func (r ParsedPipeCallResult) ArgsOutType() readonly.Slice[*checker.Type] {
	return readonly.New(r.argsOutType.get())
}

// parsedSingleArgCallResult is the internal result of parsing a single-argument call.
type parsedSingleArgCallResult struct {
	node             *ast.CallExpression // the applied call
	callee           *ast.Node           // the applied function, decomposed for curried factories
	args             []*ast.Node         // curried factory arguments, or nil
	typeArgumentsSrc *ast.Node           // the node carrying the transformation type arguments
	subject          *ast.Node
}

// ParsePipeCall detects pipe() and .pipe() call patterns.
// Returns nil when the node is not a recognized pipe call.
func (tp *TypeParser) ParsePipeCall(node *ast.Node) *ParsedPipeCallResult {
	if tp == nil || tp.checker == nil || node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}

	return Cached(&tp.links.ParsePipeCall, node, func() *ParsedPipeCallResult {
		call := node.AsCallExpression()
		if call == nil || call.Expression == nil {
			return nil
		}

		// Case 1: PropertyAccessExpression — either Namespace.pipe(...) or expr.pipe(...)
		if call.Expression.Kind == ast.KindPropertyAccessExpression {
			propAccess := call.Expression.AsPropertyAccessExpression()
			if propAccess == nil || propAccess.Name() == nil {
				return nil
			}

			nameText := scanner.GetTextOfNode(propAccess.Name())
			if nameText != "pipe" {
				return nil
			}

			// Check if this is Function.pipe from "effect" package
			if tp.IsNodeReferenceToEffectPackageExport(call.Expression, "pipe") {
				// This is pipe(subject, f1, f2, ...) via namespace access (e.g., Function.pipe)
				if call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
					return nil
				}
				return tp.buildParsedPipeCallResult(call, call.Arguments.Nodes[0], call.Arguments.Nodes[1:], TransformationKindPipe)
			}

			// Not from "effect" package — this is a .pipe() pipeable method call
			// Any .pipe() call is treated as a pipeable chain
			subject := propAccess.Expression
			if subject == nil {
				return nil
			}
			var args []*ast.Node
			if call.Arguments != nil {
				args = call.Arguments.Nodes
			}
			return tp.buildParsedPipeCallResult(call, subject, args, TransformationKindPipeable)
		}

		// Case 2: Identifier — bare pipe(subject, f1, f2, ...)
		if call.Expression.Kind == ast.KindIdentifier {
			if call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
				return nil
			}

			if !tp.IsNodeReferenceToEffectPackageExport(call.Expression, "pipe") {
				return nil
			}
			return tp.buildParsedPipeCallResult(call, call.Arguments.Nodes[0], call.Arguments.Nodes[1:], TransformationKindPipe)
		}

		return nil
	})
}

func (tp *TypeParser) buildParsedPipeCallResult(
	call *ast.CallExpression,
	subject *ast.Node,
	args []*ast.Node,
	kind TransformationKind,
) *ParsedPipeCallResult {
	return &ParsedPipeCallResult{
		node:        call,
		subject:     subject,
		args:        args,
		kind:        kind,
		subjectType: tp.lazyTypeAtLocation(subject),
		argsOutType: newLazy(func() []*checker.Type { return tp.buildPipeArgsOutType(call, len(args)) }),
	}
}

func (tp *TypeParser) buildPipeArgsOutType(call *ast.CallExpression, argCount int) []*checker.Type {
	outTypes := make([]*checker.Type, argCount)
	sig := tp.checker.GetResolvedSignature(call.AsNode())
	if sig == nil {
		return outTypes
	}
	typeArgs := tp.checker.GetTypeArgumentsForResolvedSignature(sig)
	for i := range outTypes {
		if i+1 < len(typeArgs) {
			outTypes[i] = typeArgs[i+1]
		}
	}
	return outTypes
}

// parseSingleArgCall detects single-argument call patterns like f(arg).
// Returns nil when the node is not a single-argument call.
func (tp *TypeParser) parseSingleArgCall(node *ast.Node) *parsedSingleArgCallResult {
	if node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}

	call := node.AsCallExpression()
	if call == nil || call.Expression == nil || call.Arguments == nil {
		return nil
	}

	if len(call.Arguments.Nodes) != 1 {
		return nil
	}

	// Spread elements like f(...args) should not be treated as single-arg calls
	if call.Arguments.Nodes[0].Kind == ast.KindSpreadElement {
		return nil
	}

	result := &parsedSingleArgCallResult{
		node:             call,
		callee:           call.Expression,
		typeArgumentsSrc: call.AsNode(),
		subject:          call.Arguments.Nodes[0],
	}
	tp.decomposeCurriedPipeable(result)
	return result
}

// parsedCallWithOptionalsResult is the internal result of parsing a
// callee(subject, optionals...) call.
type parsedCallWithOptionalsResult struct {
	subject *ast.Node
	args    []*ast.Node
}

// parseCallWithOptionals detects calls with at least one optional argument
// whose callee has exactly one call signature, with exactly one required
// parameter and no rest parameter. Calls with only the subject are left to
// parseSingleArgCall.
func (tp *TypeParser) parseCallWithOptionals(node *ast.Node) *parsedCallWithOptionalsResult {
	if tp == nil || tp.checker == nil || node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}
	call := node.AsCallExpression()
	if call == nil || call.Expression == nil || call.Arguments == nil || len(call.Arguments.Nodes) < 2 {
		return nil
	}
	for _, arg := range call.Arguments.Nodes {
		if arg == nil || arg.Kind == ast.KindSpreadElement {
			return nil
		}
	}

	calleeType := tp.GetTypeAtLocation(call.Expression)
	if calleeType == nil || len(tp.checker.GetSignaturesOfType(calleeType, checker.SignatureKindCall)) != 1 {
		return nil
	}
	signature := tp.checker.GetResolvedSignature(node)
	if signature == nil || signature.MinArgumentCount() != 1 || signature.HasRestParameter() ||
		len(call.Arguments.Nodes) > len(signature.Parameters()) {
		return nil
	}
	return &parsedCallWithOptionalsResult{
		subject: call.Arguments.Nodes[0],
		args:    call.Arguments.Nodes[1:],
	}
}

// decomposeCurriedPipeable exposes the factory arguments only when the inner
// call is the pipeable counterpart of a data-first overload of the same symbol.
func (tp *TypeParser) decomposeCurriedPipeable(result *parsedSingleArgCallResult) {
	if tp == nil || tp.checker == nil || result == nil || result.node == nil {
		return
	}
	factory := ast.SkipParentheses(result.node.Expression)
	if factory == nil || factory.Kind != ast.KindCallExpression {
		return
	}
	factoryCall := factory.AsCallExpression()
	if factoryCall == nil || factoryCall.Expression == nil || factoryCall.Arguments == nil || len(factoryCall.Arguments.Nodes) == 0 {
		return
	}

	c := tp.checker
	pipeable := c.GetResolvedSignature(factory)
	if pipeable == nil {
		return
	}
	pipeableDeclaration := rawSignature(pipeable).Declaration()
	if pipeableDeclaration == nil {
		return
	}
	pipeableSymbol := checker.Checker_getSymbolOfDeclaration(c, pipeableDeclaration)
	if pipeableSymbol == nil {
		return
	}

	calleeType := tp.GetTypeAtLocation(factoryCall.Expression)
	subjectType := tp.GetTypeAtLocation(result.subject)
	if calleeType == nil || subjectType == nil {
		return
	}
	argumentTypes := make([]*checker.Type, 0, len(factoryCall.Arguments.Nodes))
	for _, argument := range factoryCall.Arguments.Nodes {
		argumentTypes = append(argumentTypes, tp.GetTypeAtLocation(argument))
	}
	witness := &PipeableSignatureWitness{ArgumentTypes: argumentTypes, SubjectType: subjectType}

	for _, dataFirst := range c.GetSignaturesOfType(calleeType, checker.SignatureKindCall) {
		if dataFirst == nil || dataFirst == pipeable {
			continue
		}
		declaration := rawSignature(dataFirst).Declaration()
		if declaration == nil {
			continue
		}
		symbol := checker.Checker_getSymbolOfDeclaration(c, declaration)
		if symbol == nil || checker.Checker_getSymbolIfSameReference(c, pipeableSymbol, symbol) == nil {
			continue
		}
		parameters := dataFirst.Parameters()
		for _, subjectIndex := range []int{0, len(parameters) - 1} {
			if MatchesPipeableSignature(tp, dataFirst, pipeable, subjectIndex, witness) {
				result.callee = factoryCall.Expression
				result.args = factoryCall.Arguments.Nodes
				result.typeArgumentsSrc = factory
				return
			}
		}
	}
}

// parsedEffectFnCallResult is the internal result of parsing an Effect.fn or Effect.fnUntraced call
// with trailing transformation arguments.
type parsedEffectFnCallResult struct {
	node                *ast.CallExpression // the outer call expression
	bodyNode            *ast.Node           // function or generator argument node
	trailingArgs        []*ast.Node         // arguments after the function body
	trailingArgsOutType []*lazy[*checker.Type]
	trailingStartIndex  int                // starting arg index of trailingArgs in node.Arguments
	kind                TransformationKind // effectFn or effectFnUntraced
}

// parseEffectFnCall detects Effect.fn-family calls with trailing transformation arguments.
// It reuses the dedicated Effect.fn parsers and only adapts their results for piping-flow analysis.
func (tp *TypeParser) parseEffectFnCall(node *ast.Node) *parsedEffectFnCallResult {
	if tp == nil || tp.checker == nil || node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}

	call := node.AsCallExpression()
	if call == nil || call.Expression == nil || call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
		return nil
	}
	if result := tp.EffectFnCall(node); result != nil && len(result.pipeArguments) > 0 {
		kind := TransformationKindEffectFn
		if result.variant == EffectFnVariantFnUntraced || result.variant == EffectFnVariantFnUntracedEager {
			kind = TransformationKindEffectFnUntraced
		}
		return &parsedEffectFnCallResult{
			node:                result.call,
			bodyNode:            result.functionNode,
			trailingArgs:        result.pipeArguments,
			trailingArgsOutType: result.pipeArgsOutType,
			trailingStartIndex:  len(result.call.Arguments.Nodes) - len(result.pipeArguments),
			kind:                kind,
		}
	}
	return nil
}

// workItem represents a node to process in the PipingFlows work queue.
type workItem struct {
	node       *ast.Node
	parentFlow *PipingFlow // non-nil when traversing subject chain for flattening
}

// PipingFlows returns all piping flows found in a source file, sorted by source position.
// Each source transformation occurrence belongs to at most one returned flow.
func (tp *TypeParser) PipingFlows(sf *ast.SourceFile, includeEffectFn bool) readonly.Slice[*PipingFlow] {
	if tp == nil || tp.checker == nil || sf == nil {
		return readonly.Slice[*PipingFlow]{}
	}

	links := tp.links
	store := &links.PipingFlowsWithoutEffectFn
	if includeEffectFn {
		store = &links.PipingFlowsWithEffectFn
	}

	return readonly.New(Cached(store, sf, func() []*PipingFlow {
		var result []*PipingFlow

		// Initialize work queue with all children of the source file
		var queue []workItem
		enqueueChild := func(child *ast.Node) bool {
			queue = append(queue, workItem{node: child})
			return false
		}
		sf.AsNode().ForEachChild(enqueueChild)

		for len(queue) > 0 {
			// Pop from end (stack behavior for depth-first)
			item := queue[len(queue)-1]
			queue = queue[:len(queue)-1]

			node := item.node
			if node == nil {
				continue
			}
			if unwrapped := ast.SkipParentheses(node); unwrapped != node {
				queue = append(queue, workItem{node: unwrapped, parentFlow: item.parentFlow})
				continue
			}

			if node.Kind == ast.KindCallExpression {
				// Try Effect.fn call first (must be before pipe and singleArg)
				if includeEffectFn {
					if efnResult := tp.parseEffectFnCall(node); efnResult != nil {
						transformations, subjectType := tp.buildEffectFnTransformations(efnResult)
						flow := &PipingFlow{
							node: node,
							partial: PartialPipingFlow{
								subject: PipingFlowSubject{
									node:    node,
									outType: subjectType,
								},
								transformations: transformations,
							},
						}
						result = append(result, flow)

						// If we were building a parent flow, finalize it with this node as subject
						if item.parentFlow != nil {
							item.parentFlow.partial.subject = PipingFlowSubject{
								node:    node,
								outType: tp.lazyTypeAtLocation(node),
							}
							result = append(result, item.parentFlow)
						}

						// Queue function body argument children for independent inner flow traversal
						if efnResult.bodyNode != nil {
							efnResult.bodyNode.ForEachChild(enqueueChild)
						}
						// Queue trailing arg children for independent inner flow traversal
						for _, arg := range efnResult.trailingArgs {
							if arg != nil {
								arg.ForEachChild(enqueueChild)
							}
						}
						continue
					}
				}

				// Try pipe call
				if pipeResult := tp.ParsePipeCall(node); pipeResult != nil {
					transformations := tp.buildPipeTransformations(pipeResult)
					flowNode := pipeResult.node.AsNode()

					if item.parentFlow != nil {
						// Extend parent flow: prepend our transformations
						item.parentFlow.partial.transformations = append(transformations, item.parentFlow.partial.transformations...)
						// Continue traversing the subject for further flattening
						queue = append(queue, workItem{node: pipeResult.subject, parentFlow: item.parentFlow})
					} else {
						// Start a new flow
						newFlow := &PipingFlow{
							node: flowNode,
							partial: PartialPipingFlow{
								transformations: transformations,
							},
						}
						queue = append(queue, workItem{node: pipeResult.subject, parentFlow: newFlow})
					}

					// Queue transformation argument children for independent inner flow traversal
					for _, arg := range pipeResult.args {
						if arg != nil {
							arg.ForEachChild(enqueueChild)
						}
					}
					continue
				}

				// Try single-arg call
				if dataFirstResult := tp.DataFirstOrLastCall(node); dataFirstResult != nil {
					kind := TransformationKindDataFirst
					if dataFirstResult.subjectIndex != 0 {
						kind = TransformationKindDataLast
					}
					transformation := PipingFlowTransformation{
						kind:          kind,
						callee:        dataFirstResult.callee,
						typeArguments: pipingFlowTypeArguments(node, dataFirstResult.callee),
						args:          dataFirstResult.args,
						outType:       tp.lazyTypeAtLocation(node),
					}

					if item.parentFlow != nil {
						item.parentFlow.partial.transformations = append(
							[]PipingFlowTransformation{transformation},
							item.parentFlow.partial.transformations...,
						)
						queue = append(queue, workItem{node: dataFirstResult.subject, parentFlow: item.parentFlow})
					} else {
						newFlow := &PipingFlow{
							node: node,
							partial: PartialPipingFlow{
								transformations: []PipingFlowTransformation{transformation},
							},
						}
						queue = append(queue, workItem{node: dataFirstResult.subject, parentFlow: newFlow})
					}

					dataFirstResult.callee.ForEachChild(enqueueChild)
					for _, arg := range dataFirstResult.args {
						if arg != nil {
							arg.ForEachChild(enqueueChild)
						}
					}
					continue
				}

				if optionalsResult := tp.parseCallWithOptionals(node); optionalsResult != nil {
					callee := node.AsCallExpression().Expression
					transformation := PipingFlowTransformation{
						kind:          TransformationKindCallWithOptionals,
						callee:        callee,
						typeArguments: pipingFlowTypeArguments(node, callee),
						args:          optionalsResult.args,
						outType:       tp.lazyCallReturnType(node),
					}

					if item.parentFlow != nil {
						item.parentFlow.partial.transformations = append(
							[]PipingFlowTransformation{transformation},
							item.parentFlow.partial.transformations...,
						)
						queue = append(queue, workItem{node: optionalsResult.subject, parentFlow: item.parentFlow})
					} else {
						newFlow := &PipingFlow{
							node: node,
							partial: PartialPipingFlow{
								transformations: []PipingFlowTransformation{transformation},
							},
						}
						queue = append(queue, workItem{node: optionalsResult.subject, parentFlow: newFlow})
					}

					callee.ForEachChild(enqueueChild)
					for _, arg := range optionalsResult.args {
						arg.ForEachChild(enqueueChild)
					}
					continue
				}

				// Try single-arg call
				if singleResult := tp.parseSingleArgCall(node); singleResult != nil {
					transformation := PipingFlowTransformation{
						kind:          TransformationKindCall,
						callee:        singleResult.callee,
						typeArguments: pipingFlowTypeArguments(singleResult.typeArgumentsSrc, singleResult.callee),
						args:          singleResult.args,
						outType:       tp.lazyCallReturnType(node),
					}

					if item.parentFlow != nil {
						// Extend parent flow: prepend this transformation
						item.parentFlow.partial.transformations = append(
							[]PipingFlowTransformation{transformation},
							item.parentFlow.partial.transformations...,
						)
						// Continue traversing the subject
						queue = append(queue, workItem{node: singleResult.subject, parentFlow: item.parentFlow})
					} else {
						// Start a new flow
						newFlow := &PipingFlow{
							node: node,
							partial: PartialPipingFlow{
								transformations: []PipingFlowTransformation{transformation},
							},
						}
						queue = append(queue, workItem{node: singleResult.subject, parentFlow: newFlow})
					}

					// Queue callee children for independent inner flow traversal
					if len(singleResult.args) > 0 {
						// Curried factory: the applied function and the factory
						// arguments belong to this transformation's subtree.
						enqueueChild(singleResult.callee)
						for _, arg := range singleResult.args {
							if arg != nil {
								enqueueChild(arg)
							}
						}
					} else {
						singleResult.callee.ForEachChild(enqueueChild)
					}
					continue
				}
			}

			// Node is not a parseable pipe/call
			if item.parentFlow != nil {
				// Subject chain terminated — finalize the flow
				item.parentFlow.partial.subject = PipingFlowSubject{
					node:    node,
					outType: tp.lazyTypeAtLocation(node),
				}
				result = append(result, item.parentFlow)
			}
			// Queue children for further traversal (independent inner flows)
			node.ForEachChild(enqueueChild)
		}

		// Sort by source position
		sort.Slice(result, func(i, j int) bool {
			return result[i].node.Pos() < result[j].node.Pos()
		})

		return result
	}))
}

// LongestPipingFlowAt returns the longest normalized piping flow rooted at node.
// Unlike PipingFlows, which discovers complete flows across a source file, this
// method can start at an expression nested inside another flow without returning
// the enclosing flow.
func (tp *TypeParser) LongestPipingFlowAt(node *ast.Node, includeEffectFn bool) *PipingFlow {
	if tp == nil || tp.checker == nil || node == nil || !ast.IsExpression(node) {
		return nil
	}

	node = ast.SkipParentheses(node)
	if includeEffectFn {
		if result := tp.parseEffectFnCall(node); result != nil {
			transformations, subjectType := tp.buildEffectFnTransformations(result)
			return &PipingFlow{
				node: node,
				partial: PartialPipingFlow{
					subject: PipingFlowSubject{
						node:    node,
						outType: subjectType,
					},
					transformations: transformations,
				},
			}
		}
	}

	if result := tp.ParsePipeCall(node); result != nil {
		flow := tp.pipingFlowSubjectAt(result.subject, includeEffectFn)
		flow.node = node
		flow.partial.transformations = append(flow.partial.transformations, tp.buildPipeTransformations(result)...)
		return flow
	}

	if result := tp.DataFirstOrLastCall(node); result != nil {
		flow := tp.pipingFlowSubjectAt(result.subject, includeEffectFn)
		kind := TransformationKindDataFirst
		if result.subjectIndex != 0 {
			kind = TransformationKindDataLast
		}
		flow.node = node
		flow.partial.transformations = append(flow.partial.transformations, PipingFlowTransformation{
			kind:          kind,
			callee:        result.callee,
			typeArguments: pipingFlowTypeArguments(node, result.callee),
			args:          result.args,
			outType:       tp.lazyTypeAtLocation(node),
		})
		return flow
	}

	if result := tp.parseCallWithOptionals(node); result != nil {
		flow := tp.pipingFlowSubjectAt(result.subject, includeEffectFn)
		callee := node.AsCallExpression().Expression
		flow.node = node
		flow.partial.transformations = append(flow.partial.transformations, PipingFlowTransformation{
			kind:          TransformationKindCallWithOptionals,
			callee:        callee,
			typeArguments: pipingFlowTypeArguments(node, callee),
			args:          result.args,
			outType:       tp.lazyCallReturnType(node),
		})
		return flow
	}

	if result := tp.parseSingleArgCall(node); result != nil {
		flow := tp.pipingFlowSubjectAt(result.subject, includeEffectFn)
		flow.node = node
		flow.partial.transformations = append(flow.partial.transformations, PipingFlowTransformation{
			kind:          TransformationKindCall,
			callee:        result.callee,
			typeArguments: pipingFlowTypeArguments(result.typeArgumentsSrc, result.callee),
			args:          result.args,
			outType:       tp.lazyCallReturnType(node),
		})
		return flow
	}

	return &PipingFlow{
		node: node,
		partial: PartialPipingFlow{
			subject: PipingFlowSubject{
				node:    node,
				outType: tp.lazyTypeAtLocation(node),
			},
		},
	}
}

func (tp *TypeParser) pipingFlowSubjectAt(node *ast.Node, includeEffectFn bool) *PipingFlow {
	if flow := tp.LongestPipingFlowAt(node, includeEffectFn); flow != nil {
		return flow
	}
	return &PipingFlow{
		node: node,
		partial: PartialPipingFlow{
			subject: PipingFlowSubject{
				node:    node,
				outType: tp.lazyTypeAtLocation(node),
			},
		},
	}
}

// buildPipeTransformations builds PipingFlowTransformation slices from pipe call arguments.
func (tp *TypeParser) buildPipeTransformations(result *ParsedPipeCallResult) []PipingFlowTransformation {
	transformations := make([]PipingFlowTransformation, 0, len(result.args))
	for i, arg := range result.args {
		if arg == nil {
			continue
		}

		outType := newLazy(func() *checker.Type {
			if outTypes := result.argsOutType.get(); i < len(outTypes) {
				return outTypes[i]
			}
			return nil
		})

		var callee *ast.Node
		var args []*ast.Node

		if arg.Kind == ast.KindCallExpression {
			callExpr := arg.AsCallExpression()
			callee = callExpr.Expression
			if callExpr.Arguments != nil && len(callExpr.Arguments.Nodes) > 0 {
				args = callExpr.Arguments.Nodes
			}
		} else {
			// Constant (e.g., Effect.asVoid used as a bare reference)
			callee = arg
		}

		transformations = append(transformations, PipingFlowTransformation{
			kind:          result.kind,
			callee:        callee,
			typeArguments: pipingFlowTypeArguments(arg, callee),
			args:          args,
			outType:       outType,
		})
	}

	return transformations
}

// buildEffectFnTransformations builds PipingFlowTransformation slices from Effect.fn trailing arguments.
// It also returns the subject type (from the first transformation's input parameter type).
func (tp *TypeParser) buildEffectFnTransformations(result *parsedEffectFnCallResult) ([]PipingFlowTransformation, *lazy[*checker.Type]) {
	transformations := make([]PipingFlowTransformation, 0, len(result.trailingArgs))
	subjectType := newLazy(func() *checker.Type { return tp.effectFnSubjectType(result) })

	for i, arg := range result.trailingArgs {
		if arg == nil {
			continue
		}

		var outType *lazy[*checker.Type]
		if i < len(result.trailingArgsOutType) {
			outType = result.trailingArgsOutType[i]
		}

		var callee *ast.Node
		var args []*ast.Node

		if arg.Kind == ast.KindCallExpression {
			callExpr := arg.AsCallExpression()
			callee = callExpr.Expression
			if callExpr.Arguments != nil && len(callExpr.Arguments.Nodes) > 0 {
				args = callExpr.Arguments.Nodes
			}
		} else {
			callee = arg
		}

		transformations = append(transformations, PipingFlowTransformation{
			kind:          result.kind,
			callee:        callee,
			typeArguments: pipingFlowTypeArguments(arg, callee),
			args:          args,
			outType:       outType,
		})
	}

	return transformations, subjectType
}

// effectFnSubjectType is the first parameter type of the contextual signature
// of the first trailing transformation.
func (tp *TypeParser) effectFnSubjectType(result *parsedEffectFnCallResult) *checker.Type {
	if len(result.trailingArgs) == 0 || result.trailingArgs[0] == nil {
		return nil
	}
	c := tp.checker
	contextualType := c.GetContextualTypeForArgumentAtIndex(result.node.AsNode(), result.trailingStartIndex)
	if contextualType == nil {
		return nil
	}
	callSigs := c.GetSignaturesOfType(contextualType, checker.SignatureKindCall)
	if len(callSigs) == 0 {
		return nil
	}
	params := callSigs[0].Parameters()
	if len(params) == 0 {
		return nil
	}
	return c.GetTypeOfSymbol(params[0])
}

func callTypeArguments(node *ast.Node) *ast.NodeList {
	// Parenthesized call arguments (e.g. pipe(x, (Effect.as<...>(v)))) keep the
	// type arguments of the wrapped call.
	node = ast.SkipParentheses(node)
	if node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}
	return node.AsCallExpression().TypeArguments
}

func pipingFlowTypeArguments(node *ast.Node, callee *ast.Node) *ast.NodeList {
	if typeArguments := callTypeArguments(node); typeArguments != nil {
		return typeArguments
	}
	return callTypeArguments(callee)
}
