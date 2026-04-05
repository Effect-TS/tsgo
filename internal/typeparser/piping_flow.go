// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"sort"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// TransformationKind represents how a transformation was expressed in source code.
type TransformationKind string

const (
	TransformationKindPipe             TransformationKind = "pipe"
	TransformationKindPipeable         TransformationKind = "pipeable"
	TransformationKindDataFirst        TransformationKind = "dataFirst"
	TransformationKindDataLast         TransformationKind = "dataLast"
	TransformationKindCall             TransformationKind = "call"
	TransformationKindEffectFn         TransformationKind = "effectFn"
	TransformationKindEffectFnUntraced TransformationKind = "effectFnUntraced"
)

// PipingFlowTransformation represents a single transformation step in a piping flow.
type PipingFlowTransformation struct {
	Kind    TransformationKind // How the transformation was expressed
	Node    *ast.Node          // The full transformation node (call expression or bare callee)
	Callee  *ast.Node          // The function being applied (e.g., Effect.map)
	Args    []*ast.Node        // Arguments to the transformation, or nil for constants/single-arg calls
	OutType *checker.Type      // The resulting type after this transformation (may be nil)
}

// PipingFlowSubject is the starting expression of a piping flow.
type PipingFlowSubject struct {
	Node    *ast.Node     // The expression node
	OutType *checker.Type // The type of the subject expression (may be nil)
}

// PipingFlow represents a complete piping flow: a subject followed by transformations.
type PipingFlow struct {
	Node            *ast.Node                  // The outermost expression encompassing the entire flow
	Subject         PipingFlowSubject          // The starting expression and its type
	Transformations []PipingFlowTransformation // Ordered list of transformations
}

// ParsedPipeCallResult is the result of parsing a pipe or pipeable call.
type ParsedPipeCallResult struct {
	Node        *ast.CallExpression
	Subject     *ast.Node
	Args        []*ast.Node
	Kind        TransformationKind
	SubjectType *checker.Type
	ArgsOutType []*checker.Type
}

// parsedSingleArgCallResult is the internal result of parsing a single-argument call.
type parsedSingleArgCallResult struct {
	node    *ast.CallExpression
	callee  *ast.Node
	subject *ast.Node
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
			nameText := scanner.GetTextOfNode(call.Expression)
			if nameText != "pipe" {
				return nil
			}

			if !tp.IsNodeReferenceToEffectPackageExport(call.Expression, "pipe") {
				return nil
			}

			if call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
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
	result := &ParsedPipeCallResult{
		Node:        call,
		Subject:     subject,
		Args:        args,
		Kind:        kind,
		SubjectType: tp.GetTypeAtLocation(subject),
		ArgsOutType: make([]*checker.Type, len(args)),
	}

	sig := tp.checker.GetResolvedSignature(call.AsNode())
	if sig == nil {
		return result
	}

	typeArgs := tp.checker.GetTypeArgumentsForResolvedSignature(sig)
	for i := range args {
		if i+1 < len(typeArgs) {
			result.ArgsOutType[i] = typeArgs[i+1]
		}
	}

	return result
}

// parseSingleArgCall detects single-argument call patterns like f(arg).
// Returns nil when the node is not a single-argument call.
func parseSingleArgCall(node *ast.Node) *parsedSingleArgCallResult {
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

	return &parsedSingleArgCallResult{
		node:    call,
		callee:  call.Expression,
		subject: call.Arguments.Nodes[0],
	}
}

// parsedEffectFnCallResult is the internal result of parsing an Effect.fn or Effect.fnUntraced call
// with trailing transformation arguments.
type parsedEffectFnCallResult struct {
	node                *ast.CallExpression // the outer call expression
	bodyNode            *ast.Node           // function or generator argument node
	trailingArgs        []*ast.Node         // arguments after the function body
	trailingArgsOutType []*checker.Type
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
	if result := tp.EffectFnCall(node); result != nil && len(result.PipeArguments) > 0 {
		kind := TransformationKindEffectFn
		if result.Variant == EffectFnVariantFnUntraced || result.Variant == EffectFnVariantFnUntracedEager {
			kind = TransformationKindEffectFnUntraced
		}
		return &parsedEffectFnCallResult{
			node:                result.Call,
			bodyNode:            result.FunctionNode,
			trailingArgs:        result.PipeArguments,
			trailingArgsOutType: result.PipeArgsOutType,
			trailingStartIndex:  len(result.Call.Arguments.Nodes) - len(result.PipeArguments),
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
func (tp *TypeParser) PipingFlows(sf *ast.SourceFile, includeEffectFn bool) []*PipingFlow {
	if tp == nil || tp.checker == nil || sf == nil {
		return nil
	}
	c := tp.checker

	links := tp.links
	store := &links.PipingFlowsWithoutEffectFn
	if includeEffectFn {
		store = &links.PipingFlowsWithEffectFn
	}

	return Cached(store, sf, func() []*PipingFlow {
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

			if node.Kind == ast.KindCallExpression {
				// Try Effect.fn call first (must be before pipe and singleArg)
				if includeEffectFn {
					if efnResult := tp.parseEffectFnCall(node); efnResult != nil {
						transformations, subjectType := tp.buildEffectFnTransformations(efnResult)
						flow := &PipingFlow{
							Node: node,
							Subject: PipingFlowSubject{
								Node:    node,
								OutType: subjectType,
							},
							Transformations: transformations,
						}
						result = append(result, flow)

						// If we were building a parent flow, finalize it with this node as subject
						if item.parentFlow != nil {
							item.parentFlow.Subject = PipingFlowSubject{
								Node:    node,
								OutType: tp.GetTypeAtLocation(node),
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
					flowNode := pipeResult.Node.AsNode()

					if item.parentFlow != nil {
						// Extend parent flow: prepend our transformations
						item.parentFlow.Transformations = append(transformations, item.parentFlow.Transformations...)
						// Continue traversing the subject for further flattening
						queue = append(queue, workItem{node: pipeResult.Subject, parentFlow: item.parentFlow})
					} else {
						// Start a new flow
						newFlow := &PipingFlow{
							Node:            flowNode,
							Transformations: transformations,
						}
						queue = append(queue, workItem{node: pipeResult.Subject, parentFlow: newFlow})
					}

					// Queue transformation argument children for independent inner flow traversal
					for _, arg := range pipeResult.Args {
						if arg != nil {
							arg.ForEachChild(enqueueChild)
						}
					}
					continue
				}

				// Try single-arg call
				if dataFirstResult := tp.ParseDataFirstCallAsPipeable(node); dataFirstResult != nil {
					callOutType := tp.GetTypeAtLocation(node)
					kind := TransformationKindDataFirst
					if dataFirstResult.SubjectIndex != 0 {
						kind = TransformationKindDataLast
					}
					transformation := PipingFlowTransformation{
						Kind:    kind,
						Node:    node,
						Callee:  dataFirstResult.Callee,
						Args:    dataFirstResult.Args,
						OutType: callOutType,
					}

					if item.parentFlow != nil {
						item.parentFlow.Transformations = append(
							[]PipingFlowTransformation{transformation},
							item.parentFlow.Transformations...,
						)
						queue = append(queue, workItem{node: dataFirstResult.Subject, parentFlow: item.parentFlow})
					} else {
						newFlow := &PipingFlow{
							Node:            node,
							Transformations: []PipingFlowTransformation{transformation},
						}
						queue = append(queue, workItem{node: dataFirstResult.Subject, parentFlow: newFlow})
					}

					dataFirstResult.Callee.ForEachChild(enqueueChild)
					for _, arg := range dataFirstResult.Args {
						if arg != nil {
							arg.ForEachChild(enqueueChild)
						}
					}
					continue
				}

				// Try single-arg call
				if singleResult := parseSingleArgCall(node); singleResult != nil {
					var callOutType *checker.Type
					if callSig := c.GetResolvedSignature(node); callSig != nil {
						callOutType = c.GetReturnTypeOfSignature(callSig)
					}
					transformation := PipingFlowTransformation{
						Kind:    TransformationKindCall,
						Node:    node,
						Callee:  singleResult.callee,
						Args:    nil,
						OutType: callOutType,
					}

					if item.parentFlow != nil {
						// Extend parent flow: prepend this transformation
						item.parentFlow.Transformations = append(
							[]PipingFlowTransformation{transformation},
							item.parentFlow.Transformations...,
						)
						// Continue traversing the subject
						queue = append(queue, workItem{node: singleResult.subject, parentFlow: item.parentFlow})
					} else {
						// Start a new flow
						newFlow := &PipingFlow{
							Node:            node,
							Transformations: []PipingFlowTransformation{transformation},
						}
						queue = append(queue, workItem{node: singleResult.subject, parentFlow: newFlow})
					}

					// Queue callee children for independent inner flow traversal
					singleResult.callee.ForEachChild(enqueueChild)
					continue
				}
			}

			// Node is not a parseable pipe/call
			if item.parentFlow != nil {
				// Subject chain terminated — finalize the flow
				item.parentFlow.Subject = PipingFlowSubject{
					Node:    node,
					OutType: tp.GetTypeAtLocation(node),
				}
				result = append(result, item.parentFlow)
			}
			// Queue children for further traversal (independent inner flows)
			node.ForEachChild(enqueueChild)
		}

		// Sort by source position
		sort.Slice(result, func(i, j int) bool {
			return result[i].Node.Pos() < result[j].Node.Pos()
		})

		return result
	})
}

// buildPipeTransformations builds PipingFlowTransformation slices from pipe call arguments.
func (tp *TypeParser) buildPipeTransformations(result *ParsedPipeCallResult) []PipingFlowTransformation {
	transformations := make([]PipingFlowTransformation, 0, len(result.Args))
	for i, arg := range result.Args {
		if arg == nil {
			continue
		}

		var outType *checker.Type
		if i < len(result.ArgsOutType) {
			outType = result.ArgsOutType[i]
		}

		var callee *ast.Node
		transformationNode := arg
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
			Kind:    result.Kind,
			Node:    transformationNode,
			Callee:  callee,
			Args:    args,
			OutType: outType,
		})
	}

	return transformations
}

// buildEffectFnTransformations builds PipingFlowTransformation slices from Effect.fn trailing arguments.
// It also returns the subject type (from the first transformation's input parameter type).
func (tp *TypeParser) buildEffectFnTransformations(result *parsedEffectFnCallResult) ([]PipingFlowTransformation, *checker.Type) {
	c := tp.checker
	transformations := make([]PipingFlowTransformation, 0, len(result.trailingArgs))
	var subjectType *checker.Type

	callNode := result.node.AsNode()

	for i, arg := range result.trailingArgs {
		if arg == nil {
			continue
		}

		// Get the contextual type of the argument within the Effect.fn call.
		argIndex := result.trailingStartIndex + i
		contextualType := c.GetContextualTypeForArgumentAtIndex(callNode, argIndex)

		var outType *checker.Type
		if i < len(result.trailingArgsOutType) {
			outType = result.trailingArgsOutType[i]
		}
		if contextualType != nil {
			callSigs := c.GetSignaturesOfType(contextualType, checker.SignatureKindCall)
			if len(callSigs) > 0 {
				// For the first transformation, extract the subject type from the first parameter
				if i == 0 {
					params := callSigs[0].Parameters()
					if len(params) > 0 {
						subjectType = c.GetTypeOfSymbol(params[0])
					}
				}
			}
		}

		var callee *ast.Node
		transformationNode := arg
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
			Kind:    result.kind,
			Node:    transformationNode,
			Callee:  callee,
			Args:    args,
			OutType: outType,
		})
	}

	return transformations, subjectType
}
