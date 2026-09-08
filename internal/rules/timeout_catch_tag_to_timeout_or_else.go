package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var TimeoutCatchTagToTimeoutOrElse = rule.Rule{
	Name:            "timeoutCatchTagToTimeoutOrElse",
	Group:           "style",
	Description:     "Suggests dedicated timeout combinators instead of catching TimeoutError immediately after Effect.timeout",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.Use_Effect_0_to_handle_this_timeout_directly_effect_timeoutCatchTagToTimeoutOrElse.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diagnostics []*ast.Diagnostic
		for _, match := range AnalyzeTimeoutCatchTagToTimeoutOrElse(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
			diagnostics = append(diagnostics, ctx.NewDiagnostic(ctx.SourceFile, match.Location, tsdiag.Use_Effect_0_to_handle_this_timeout_directly_effect_timeoutCatchTagToTimeoutOrElse, nil, match.ReplacementName))
		}
		return diagnostics
	},
}

type TimeoutCatchTagToTimeoutOrElseMatch struct {
	Flow                            *typeparser.PipingFlow
	Start, End                      int
	Location                        core.TextRange
	ReplacementName                 string
	EffectModule, Duration, Handler *ast.Node
}

func AnalyzeTimeoutCatchTagToTimeoutOrElse(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []TimeoutCatchTagToTimeoutOrElseMatch {
	if tp == nil || c == nil || sf == nil || tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	var matches []TimeoutCatchTagToTimeoutOrElseMatch
	// Check both alternatives in one pass: collecting and sorting matches per
	// pattern adds work and allocations to this frequently run analysis.
	for _, flow := range tp.PipingFlows(sf, true) {
		for i := 0; i+1 < len(flow.Transformations); i++ {
			timeout := &flow.Transformations[i]
			if len(timeout.Args) != 1 || !tp.IsNodeReferenceToEffectModuleApi(timeout.Callee, "timeout") {
				continue
			}
			end := i + 1
			option := timeoutAsSome(tp, &flow.Transformations[end])
			if option {
				end++
			}
			if end >= len(flow.Transformations) {
				continue
			}
			catch := &flow.Transformations[end]
			if len(catch.Args) != 2 || !tp.IsNodeReferenceToEffectModuleApi(catch.Callee, "catchTag") {
				continue
			}
			tag := ast.SkipParentheses(catch.Args[0])
			if tag.Kind != ast.KindStringLiteral || tag.AsStringLiteral().Text != "TimeoutError" {
				continue
			}
			handler := ast.SkipParentheses(catch.Args[1])
			if !timeoutIgnoresError(tp, c, handler) {
				continue
			}
			if option && !timeoutReturnsNone(tp, handler) {
				continue
			}
			input := tp.StrictEffectType(flow.TransformationInputType(i))
			if input == nil || !timeoutErrorExcluded(tp, c, input.E) {
				continue
			}
			hasTypeArgs := false
			for j := i; j <= end; j++ {
				hasTypeArgs = hasTypeArgs || catchDieHasTypeArguments(&flow.Transformations[j])
			}
			if hasTypeArgs {
				continue
			}
			name := "timeoutOrElse"
			if option {
				name = "timeoutOption"
			}
			var module *ast.Node
			if catch.Callee.Kind == ast.KindPropertyAccessExpression {
				module = catch.Callee.AsPropertyAccessExpression().Expression
			}
			matches = append(matches, TimeoutCatchTagToTimeoutOrElseMatch{Flow: flow, Start: i, End: end, Location: scanner.GetErrorRangeForNode(sf, catch.Callee), ReplacementName: name, EffectModule: module, Duration: timeout.Args[0], Handler: catch.Args[1]})
		}
	}
	return matches
}
func timeoutAsSome(tp *typeparser.TypeParser, step *typeparser.PipingFlowTransformation) bool {
	return len(step.Args) == 0 && tp.IsNodeReferenceToEffectModuleApi(step.Callee, "asSome") ||
		len(step.Args) == 1 && tp.IsNodeReferenceToEffectModuleApi(step.Callee, "map") && tp.IsNodeReferenceToEffectOptionModuleApi(step.Args[0], "some")
}

func timeoutReturnsNone(tp *typeparser.TypeParser, handler *ast.Node) bool {
	lazy := typeparser.ParseLazyExpression(handler, typeparser.LazyExpressionNone)
	if lazy == nil {
		return false
	}
	flow := tp.LongestPipingFlowAt(lazy.Expression, false)
	return flow.MatchesExactly(func(subject *typeparser.PipingFlowSubject) bool {
		return tp.IsNodeReferenceToEffectModuleApi(subject.Node, "succeedNone")
	}) || flow.MatchesExactly(
		func(subject *typeparser.PipingFlowSubject) bool {
			return isOptionNoneCall(tp, subject.Node)
		},
		func(step *typeparser.PipingFlowTransformation) bool {
			return len(step.Args) == 0 && (step.TypeArguments == nil || len(step.TypeArguments.Nodes) == 0) &&
				tp.IsNodeReferenceToEffectModuleApi(step.Callee, "succeed")
		},
	)
}

// Fail closed for unknown, generic, or broadly tagged errors: catchTag matches
// structurally by tag, so checking only the canonical TimeoutError is unsafe.
func timeoutErrorExcluded(tp *typeparser.TypeParser, c *checker.Checker, errorType *checker.Type) bool {
	if errorType == nil {
		return false
	}
	if errorType.Flags()&checker.TypeFlagsNever != 0 {
		return true
	}
	for _, member := range tp.UnrollUnionMembers(errorType) {
		tag := c.GetTypeOfPropertyOfType(member, "_tag")
		if tag == nil {
			return false
		}
		for _, value := range tp.UnrollUnionMembers(tag) {
			if value.Flags()&checker.TypeFlagsStringLiteral == 0 {
				return false
			}
			if text, ok := value.AsLiteralType().Value().(string); !ok || text == "TimeoutError" {
				return false
			}
		}
	}
	return true
}

// Restrict callbacks to arrows so the caught error cannot be observed through
// a function's arguments object. An unused plain parameter can be removed.
func timeoutIgnoresError(tp *typeparser.TypeParser, c *checker.Checker, handler *ast.Node) bool {
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
		return node.ForEachChild(usesParameter)
	}
	return !usesParameter(handler.AsArrowFunction().Body) && (handler.AsArrowFunction().Type == nil || !usesParameter(handler.AsArrowFunction().Type))
}
