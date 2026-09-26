package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// CatchRefailToTapError recognizes observation followed by an unchanged refail.
var CatchRefailToTapError = rule.Rule{
	Name:            "catchRefailToTapError",
	Group:           "style",
	Description:     "Suggests Effect.tapError for catch handlers that sequence an effect and then re-fail the original error",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.Use_Effect_tapError_to_observe_the_error_and_preserve_the_original_failure_without_having_to_re_fail_it_effect_catchRefailToTapError.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		tp := ctx.TypeParser
		if tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
			return nil
		}
		var diagnostics []*ast.Diagnostic
		for _, flow := range tp.PipingFlows(ctx.SourceFile, true) {
			for i := range flow.Transformations {
				step := &flow.Transformations[i]
				if len(step.Args) != 1 || !tp.IsNodeReferenceToEffectModuleApi(step.Callee, "catch") {
					continue
				}
				input := tp.StrictEffectType(flow.TransformationInputType(i))
				if input == nil || input.E == nil || input.E.Flags()&checker.TypeFlagsNever != 0 ||
					!catchHandlerRefailsOriginalError(tp, ctx.Checker, step.Args[0]) {
					continue
				}
				diagnostics = append(diagnostics, ctx.NewDiagnostic(
					ctx.SourceFile,
					scanner.GetErrorRangeForNode(ctx.SourceFile, step.Callee),
					tsdiag.Use_Effect_tapError_to_observe_the_error_and_preserve_the_original_failure_without_having_to_re_fail_it_effect_catchRefailToTapError,
					nil,
				))
			}
		}
		return diagnostics
	},
}

func catchHandlerRefailsOriginalError(tp *typeparser.TypeParser, c *checker.Checker, handler *ast.Node) bool {
	lazy := typeparser.ParseLazyExpression(handler, typeparser.LazyExpressionNone)
	if lazy == nil || len(lazy.Params) != 1 {
		return false
	}
	parameter := lazy.Params[0].AsParameterDeclaration()
	if parameter.Name() == nil || parameter.Name().Kind != ast.KindIdentifier ||
		parameter.Initializer != nil || parameter.DotDotDotToken != nil {
		return false
	}
	errorSymbol := tp.GetSymbolAtLocation(parameter.Name())
	if errorSymbol == nil || checker.Checker_isSymbolAssigned(c, errorSymbol) {
		return false
	}

	flow := tp.LongestPipingFlowAt(lazy.Expression, false)
	if flow == nil || len(flow.Transformations) == 0 {
		return false
	}
	last := len(flow.Transformations) - 1
	step := &flow.Transformations[last]
	if len(step.Args) != 1 || tp.StrictEffectType(flow.TransformationInputType(last)) == nil {
		return false
	}

	refail := step.Args[0]
	switch {
	case tp.IsNodeReferenceToEffectModuleApi(step.Callee, "andThen"):
		// andThen accepts both an Effect and a callback. Only a zero-argument
		// callback can be discarded without binding the preceding success value.
		if thunk := typeparser.ParseLazyExpression(refail, typeparser.LazyExpressionThunk); thunk != nil {
			refail = thunk.Expression
		}
	case tp.IsNodeReferenceToEffectModuleApi(step.Callee, "flatMap"):
		thunk := typeparser.ParseLazyExpression(refail, typeparser.LazyExpressionThunk)
		if thunk == nil {
			return false
		}
		refail = thunk.Expression
	default:
		return false
	}

	// Matching the whole inner flow excludes recovery or other work after fail,
	// and excludes expressions which merely compute a different error value.
	return tp.LongestPipingFlowAt(refail, false).MatchesExactly(
		func(subject *typeparser.PipingFlowSubject) bool {
			node := ast.SkipParentheses(subject.Node)
			return node != nil && node.Kind == ast.KindIdentifier && tp.GetSymbolAtLocation(node) == errorSymbol
		},
		func(step *typeparser.PipingFlowTransformation) bool {
			return len(step.Args) == 0 && tp.IsNodeReferenceToEffectModuleApi(step.Callee, "fail")
		},
	)
}
