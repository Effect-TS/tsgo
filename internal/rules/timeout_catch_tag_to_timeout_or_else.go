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
	Flow              *typeparser.PipingFlow
	Start, End        int
	Location          core.TextRange
	ReplacementName   string
	Duration, Handler *ast.Node
}

func AnalyzeTimeoutCatchTagToTimeoutOrElse(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []TimeoutCatchTagToTimeoutOrElseMatch {
	if tp == nil || c == nil || sf == nil || tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	var matches []TimeoutCatchTagToTimeoutOrElseMatch
	// Check both alternatives in one pass: collecting and sorting matches per
	// pattern adds work and allocations to this frequently run analysis.
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for i := 0; i+1 < flow.Transformations().Len(); i++ {
			timeout := flow.Transformations().At(i)
			if timeout.Args().Len() != 1 || !tp.IsNodeReferenceToEffectModuleApi(timeout.Callee(), "timeout") {
				continue
			}
			end := i + 1
			option := timeoutAsSome(tp, new(flow.Transformations().At(end)))
			if option {
				end++
			}
			if end >= flow.Transformations().Len() {
				continue
			}
			catch := flow.Transformations().At(end)
			if catch.Args().Len() != 2 || !tp.IsNodeReferenceToEffectModuleApi(catch.Callee(), "catchTag") {
				continue
			}
			tag := ast.SkipParentheses(catch.Args().At(0))
			if tag.Kind != ast.KindStringLiteral || tag.AsStringLiteral().Text != "TimeoutError" {
				continue
			}
			hasTypeArgs := false
			for j := i; j <= end; j++ {
				hasTypeArgs = hasTypeArgs || hasExplicitTypeArguments(new(flow.Transformations().At(j)))
			}
			if hasTypeArgs {
				continue
			}
			handler := ast.SkipParentheses(catch.Args().At(1))
			if !catchHandlerIgnoresError(tp, c, handler) {
				continue
			}
			if option && !catchHandlerReturnsSucceedNone(tp, handler) {
				continue
			}
			input := tp.StrictEffectType(flow.TransformationInputType(i))
			if input == nil || !timeoutErrorExcluded(tp, c, input.E()) {
				continue
			}
			name := "timeoutOrElse"
			if option {
				name = "timeoutOption"
			}
			matches = append(matches, TimeoutCatchTagToTimeoutOrElseMatch{Flow: flow, Start: i, End: end, Location: scanner.GetErrorRangeForNode(sf, catch.Callee()), ReplacementName: name, Duration: timeout.Args().At(0), Handler: catch.Args().At(1)})
		}
	}
	return matches
}
func timeoutAsSome(tp *typeparser.TypeParser, step *typeparser.PipingFlowTransformation) bool {
	return step.Args().Len() == 0 && tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "asSome") ||
		step.Args().Len() == 1 && tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "map") && tp.IsNodeReferenceToEffectOptionModuleApi(step.Args().At(0), "some")
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
