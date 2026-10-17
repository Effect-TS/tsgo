package typeparser

import (
	"slices"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

func (tp *TypeParser) CachedObjectLiteralContextualType(node *ast.Node) *checker.Type {
	if tp == nil || tp.checker == nil || node == nil || node.Kind != ast.KindObjectLiteralExpression {
		return nil
	}
	return Cached(&tp.links.ObjectLiteralContextualType, node, func() *checker.Type {
		return tp.GetContextualType(node, checker.ContextFlagsNone)
	})
}

// CachedObjectLiteralConstraintContextualType returns the contextual type of an
// object literal inside a generic call argument, as if the argument contributed
// no inferences to the call's resolved signature. Type parameters then take
// their return type inferences, inferences from the other arguments, their
// defaults, or their constraints, as the checker infers them. It returns nil
// when the literal is not inside a call argument or the signature is not
// generic.
//
// The call is never resolved again. An inference-blocked contextual query
// (ContextFlagsIgnoreNodeInferences) would drop the resolved signatures of the
// enclosing calls and resolve them with the argument's inferences removed. On
// the shared checker this can instantiate types the original check never
// built, for example Match.when patterns over a wide union, and leak
// TS2589 errors and error types into the program's diagnostics.
func (tp *TypeParser) CachedObjectLiteralConstraintContextualType(node *ast.Node) *checker.Type {
	if tp == nil || tp.checker == nil || node == nil || node.Kind != ast.KindObjectLiteralExpression {
		return nil
	}
	return Cached(&tp.links.ObjectLiteralConstraintContextualType, node, func() *checker.Type {
		return tp.objectLiteralConstraintContextualType(node)
	})
}

func (tp *TypeParser) objectLiteralConstraintContextualType(node *ast.Node) *checker.Type {
	levels := tp.inferenceReplayLevels(node)
	if len(levels) == 0 {
		return nil
	}
	c := tp.checker
	// Each enclosing generic call was inferred with the literal's influence, so
	// replay them from the outermost inwards. The pushed argument type of an
	// outer call is what the next call sees as its contextual type.
	pushed := 0
	defer func() {
		for range pushed {
			checker.Checker_popContextualType(c)
		}
	}()
	for _, level := range slices.Backward(levels) {
		argumentType := tp.argumentTypeWithoutOwnInferences(level)
		if argumentType == nil {
			return nil
		}
		checker.Checker_pushContextualType(c, level.arguments[level.index], argumentType, false /*isCache*/)
		pushed++
	}
	return tp.GetContextualType(node, checker.ContextFlagsNone)
}

// inferenceReplayLevel is a generic call whose argument at index contains the
// literal, with the checked types of its receiver and other arguments. A call
// with explicit type arguments has a fixed parameter type instead.
type inferenceReplayLevel struct {
	call          *ast.Node
	target        *checker.Signature
	arguments     []*ast.Node
	index         int
	thisType      *checker.Type
	argumentTypes []*checker.Type
	fixed         *checker.Type
}

// inferenceReplayLevels returns the generic calls enclosing node, innermost
// first, stopping at the first enclosing call that is not generic or has
// explicit type arguments. It returns nil when the nearest call is not generic.
// The argument types are read before any contextual type is pushed, because the
// checker caches them.
func (tp *TypeParser) inferenceReplayLevels(node *ast.Node) []inferenceReplayLevel {
	c := tp.checker
	var levels []inferenceReplayLevel
	for current := node; ; {
		call, arguments, index := tp.containingCallArgument(current)
		if call == nil {
			return levels
		}
		signature := c.GetResolvedSignature(call)
		if signature == nil || signature.Target() == nil || len(signature.Target().TypeParameters()) == 0 {
			return levels
		}
		if len(call.TypeArguments()) != 0 {
			return append(levels, inferenceReplayLevel{arguments: arguments, index: index, fixed: c.GetTypeAtPosition(signature, index)})
		}
		var thisType *checker.Type
		if checker.Checker_getThisTypeOfSignature(c, signature.Target()) != nil {
			thisType = checker.Checker_getThisArgumentType(c, checker.Checker_getThisArgumentOfCall(c, call))
		}
		argumentTypes := make([]*checker.Type, len(arguments))
		for i, other := range arguments {
			if i != index && other.Kind != ast.KindOmittedExpression {
				argumentTypes[i] = checker.Checker_checkExpressionCached(c, other)
			}
		}
		levels = append(levels, inferenceReplayLevel{call: call, target: signature.Target(), arguments: arguments, index: index, thisType: thisType, argumentTypes: argumentTypes})
		current = call
	}
}

// argumentTypeWithoutOwnInferences replays the checker's inferTypeArguments for
// the level's call, skipping the argument that contains the literal, and
// returns that argument's parameter type under the replayed inferences. The
// mapper resolves only the type parameters the parameter type needs.
func (tp *TypeParser) argumentTypeWithoutOwnInferences(level inferenceReplayLevel) *checker.Type {
	if level.fixed != nil {
		return level.fixed
	}
	c := tp.checker
	target := level.target
	typeParameters := target.TypeParameters()
	context := checker.Checker_newInferenceContext(c, typeParameters, target, checker.InferenceFlagsNone, nil)
	inferences := checker.InferenceContext_inferences(context)
	skipBindingPatterns := !slices.ContainsFunc(typeParameters, func(typeParameter *checker.Type) bool {
		return c.GetDefaultFromTypeParameter(typeParameter) == nil
	})
	contextFlags := checker.ContextFlagsNone
	if skipBindingPatterns {
		contextFlags = checker.ContextFlagsSkipBindingPatterns
	}
	if contextualType := tp.GetContextualType(level.call, contextFlags); contextualType != nil {
		isFromBindingPattern := !skipBindingPatterns && tp.GetContextualType(level.call, checker.ContextFlagsSkipBindingPatterns) != contextualType
		if !isFromBindingPattern {
			source := contextualType
			if contextualSignature := checker.Checker_getSingleCallSignature(c, contextualType); contextualSignature != nil && len(contextualSignature.TypeParameters()) != 0 {
				source = checker.Checker_getOrCreateTypeFromSignature(c, checker.Checker_getSignatureInstantiationWithoutFillingInTypeArguments(c, contextualSignature, contextualSignature.TypeParameters()))
			}
			checker.Checker_inferTypes(c, inferences, source, c.GetReturnTypeOfSignature(target), checker.InferencePriorityReturnType, false /*contravariant*/)
		}
	}
	if level.thisType != nil {
		checker.Checker_inferTypes(c, inferences, level.thisType, checker.Checker_getThisTypeOfSignature(c, target), checker.InferencePriorityNone, false /*contravariant*/)
	}
	for i, argumentType := range level.argumentTypes {
		if argumentType == nil {
			continue
		}
		if parameterType := c.GetTypeAtPosition(target, i); parameterType != nil {
			checker.Checker_inferTypes(c, inferences, argumentType, parameterType, checker.InferencePriorityNone, false /*contravariant*/)
		}
	}
	parameterType := c.GetTypeAtPosition(target, level.index)
	if parameterType == nil {
		return nil
	}
	return checker.Checker_instantiateType(c, parameterType, checker.Checker_getMapperFromContext(c, context))
}

// CachedObjectLiteralBareConstraintContextualType returns the contextual type of
// an object literal inside a generic call argument, as if each constrained type
// parameter of the call's resolved signature were its constraint. Constraints
// may mention other type parameters, so they are instantiated with the resolved
// inferences. An unconstrained type parameter may have been inferred from the
// literal, so it becomes unknown. It returns nil when the literal is not inside
// a call argument or no type parameter is constrained. Like
// CachedObjectLiteralConstraintContextualType, it never resolves the call
// again.
func (tp *TypeParser) CachedObjectLiteralBareConstraintContextualType(node *ast.Node) *checker.Type {
	if tp == nil || tp.checker == nil || node == nil || node.Kind != ast.KindObjectLiteralExpression {
		return nil
	}
	return Cached(&tp.links.ObjectLiteralBareConstraintContextualType, node, func() *checker.Type {
		return tp.objectLiteralBareConstraintContextualType(node)
	})
}

func (tp *TypeParser) objectLiteralBareConstraintContextualType(node *ast.Node) *checker.Type {
	call, arguments, index := tp.containingCallArgument(node)
	if call == nil {
		return nil
	}
	c := tp.checker
	signature := c.GetResolvedSignature(call)
	if signature == nil || signature.Target() == nil {
		return nil
	}
	target := signature.Target()
	typeParameters := target.TypeParameters()
	typeArguments := c.GetTypeArgumentsForResolvedSignature(signature)
	if len(typeParameters) == 0 || len(typeArguments) != len(typeParameters) {
		return nil
	}
	inferred := checker.NewTypeMapper(typeParameters, typeArguments)
	constrained := make([]*checker.Type, len(typeParameters))
	hasConstraint := false
	for i, typeParameter := range typeParameters {
		if constraint := c.GetConstraintOfTypeParameter(typeParameter); constraint != nil {
			constrained[i] = checker.Checker_instantiateType(c, constraint, inferred)
			hasConstraint = true
		} else {
			constrained[i] = c.GetUnknownType()
		}
	}
	if !hasConstraint {
		return nil
	}
	parameterType := c.GetTypeAtPosition(target, index)
	if parameterType == nil {
		return nil
	}
	argumentType := checker.Checker_instantiateType(c, parameterType, checker.NewTypeMapper(typeParameters, constrained))
	checker.Checker_pushContextualType(c, arguments[index], argumentType, false /*isCache*/)
	defer checker.Checker_popContextualType(c)
	return tp.GetContextualType(node, checker.ContextFlagsNone)
}

// containingCallArgument returns the nearest enclosing call whose arguments
// contain node, its effective arguments, and the index of the argument that
// contains node. It returns a nil call when there is none, or when a spread
// argument comes before or at that argument.
func (tp *TypeParser) containingCallArgument(node *ast.Node) (*ast.Node, []*ast.Node, int) {
	call := containingCall(node)
	if call == nil {
		return nil, nil, -1
	}
	arguments := checker.Checker_getEffectiveCallArguments(tp.checker, call)
	argument := ast.FindAncestor(node, func(n *ast.Node) bool { return n == call || slices.Contains(arguments, n) })
	index := slices.Index(arguments, argument)
	if index < 0 || slices.ContainsFunc(arguments[:index+1], ast.IsSpreadElement) {
		return nil, nil, -1
	}
	return call, arguments, index
}

// containingCall returns the nearest enclosing call, new expression, or tagged
// template whose arguments contain node.
func containingCall(node *ast.Node) *ast.Node {
	for current := node; current.Parent != nil; current = current.Parent {
		parent := current.Parent
		if ast.IsCallLikeExpression(parent) {
			if ast.IsCallOrNewExpression(parent) && current != parent.Expression() ||
				ast.IsTaggedTemplateExpression(parent) && current != parent.AsTaggedTemplateExpression().Tag {
				return parent
			}
			return nil
		}
	}
	return nil
}
