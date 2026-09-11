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

// FlatMapIgnoredParamToAndThen suggests Effect.andThen when an Effect.flatMap
// callback ignores the upstream value and returns an existing Effect value.
var FlatMapIgnoredParamToAndThen = rule.Rule{
	Name:            "flatMapIgnoredParamToAndThen",
	Group:           "style",
	Description:     "Suggests using Effect.andThen instead of Effect.flatMap when a zero-parameter callback returns an existing Effect value",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_andThen_expresses_this_sequencing_more_directly_than_Effect_flatMap_with_a_zero_parameter_callback_effect_flatMapIgnoredParamToAndThen.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeFlatMapIgnoredParamToAndThen(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_andThen_expresses_this_sequencing_more_directly_than_Effect_flatMap_with_a_zero_parameter_callback_effect_flatMapIgnoredParamToAndThen,
				nil,
			)
		}
		return diagnostics
	},
}

// FlatMapIgnoredParamToAndThenMatch holds the nodes needed by the diagnostic
// and quick fix.
type FlatMapIgnoredParamToAndThenMatch struct {
	SourceFile     *ast.SourceFile
	Location       core.TextRange
	Callee         *ast.Node
	CalleeNameNode *ast.Node
	Callback       *ast.Node
	EffectValue    *ast.Node
}

// AnalyzeFlatMapIgnoredParamToAndThen finds Effect.flatMap piping
// transformations whose zero-parameter expression callback returns an Effect
// value held by an already-initialized const binding.
func AnalyzeFlatMapIgnoredParamToAndThen(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []FlatMapIgnoredParamToAndThenMatch {
	var matches []FlatMapIgnoredParamToAndThenMatch

	for _, flow := range tp.PipingFlows(sf, true) {
		for _, transformation := range flow.Transformations {
			callee := transformation.Callee
			if callee == nil || len(transformation.Args) != 1 ||
				!tp.IsNodeReferenceToEffectModuleApi(callee, "flatMap") {
				continue
			}

			callback := typeparser.ParseLazyExpression(transformation.Args[0], typeparser.LazyExpressionThunk)
			if callback == nil || callback.Expression == nil ||
				!tp.IsExpressionValueStableAtLocation(callback.Expression, callee) ||
				tp.EffectType(tp.GetTypeAtLocation(callback.Expression)) == nil {
				continue
			}

			var calleeName *ast.Node
			if callee.Kind == ast.KindPropertyAccessExpression {
				calleeName = callee.AsPropertyAccessExpression().Name()
			}

			matches = append(matches, FlatMapIgnoredParamToAndThenMatch{
				SourceFile:     sf,
				Location:       scanner.GetErrorRangeForNode(sf, callee),
				Callee:         callee,
				CalleeNameNode: calleeName,
				Callback:       callback.Node,
				EffectValue:    callback.Expression,
			})
		}
	}

	return matches
}
