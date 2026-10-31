package rules

import (
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// Restrict callbacks to arrows so the caught error cannot be observed through
// a function's arguments object. An unused plain parameter can be removed.
func catchHandlerIgnoresError(tp *typeparser.TypeParser, c *checker.Checker, handler *ast.Node) bool {
	if handler == nil || handler.Kind != ast.KindArrowFunction || ast.GetCombinedModifierFlags(handler)&ast.ModifierFlagsAsync != 0 {
		return false
	}
	if params := typeparser.GetFunctionLikeTypeParameters(handler); params != nil && len(params.Nodes) != 0 {
		return false
	}
	params := typeparser.GetFunctionLikeParameters(handler)
	if params == nil || len(params.Nodes) == 0 {
		return true
	}
	if len(params.Nodes) != 1 {
		return false
	}
	parameter := params.Nodes[0].AsParameterDeclaration()
	if parameter == nil || parameter.Name().Kind != ast.KindIdentifier || parameter.Initializer != nil || parameter.DotDotDotToken != nil {
		return false
	}
	symbol := tp.GetSymbolAtLocation(parameter.Name())
	if symbol == nil {
		return false
	}
	var usesParameter func(*ast.Node) bool
	usesParameter = func(node *ast.Node) bool {
		if node.Kind == ast.KindShorthandPropertyAssignment && c.GetShorthandAssignmentValueSymbol(node) == symbol {
			return true
		}
		if node.Kind == ast.KindIdentifier && tp.GetSymbolAtLocation(node) == symbol {
			return true
		}
		//ets:ast-walk scans the handler body and return type for references to the caught error
		return node.ForEachChild(usesParameter)
	}
	return !usesParameter(handler.AsArrowFunction().Body) && (handler.AsArrowFunction().Type == nil || !usesParameter(handler.AsArrowFunction().Type))
}

// catchHandlerReturnsSucceedNone reports whether a catch handler's complete
// returned flow is Effect.succeedNone or Effect.succeed(Option.none()).
func catchHandlerReturnsSucceedNone(tp *typeparser.TypeParser, handler *ast.Node) bool {
	lazy := typeparser.ParseLazyExpression(handler, typeparser.LazyExpressionNone)
	if lazy == nil {
		return false
	}
	flow := tp.LongestPipingFlowAt(lazy.Expression(), false)
	return flow.MatchesExactly(func(subject typeparser.PipingFlowSubject) bool {
		return tp.IsNodeReferenceToEffectModuleApi(subject.Node(), "succeedNone")
	}) || flow.MatchesExactly(
		func(subject typeparser.PipingFlowSubject) bool {
			return isOptionNoneCall(tp, subject.Node())
		},
		func(step typeparser.PipingFlowTransformation) bool {
			return step.Args().Len() == 0 && (step.TypeArguments() == nil || len(step.TypeArguments().Nodes) == 0) &&
				tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "succeed")
		},
	)
}
