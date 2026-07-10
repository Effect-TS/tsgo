// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// FlatMapToMap suggests using Effect.map when an Effect.flatMap callback only
// wraps its result with Effect.succeed.
var FlatMapToMap = rule.Rule{
	Name:            "flatMapToMap",
	Group:           "style",
	Description:     "Suggests using Effect.map instead of Effect.flatMap when the callback only wraps its result with Effect.succeed",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_map_expresses_this_success_value_transformation_more_directly_than_Effect_flatMap_followed_by_Effect_succeed_effect_flatMapToMap.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeFlatMapToMap(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_map_expresses_this_success_value_transformation_more_directly_than_Effect_flatMap_followed_by_Effect_succeed_effect_flatMapToMap,
				nil,
			)
		}
		return diags
	},
}

// FlatMapToMapMatch holds the AST nodes needed by the diagnostic and quick fix.
type FlatMapToMapMatch struct {
	SourceFile            *ast.SourceFile
	Location              core.TextRange
	CalleeNameNode        *ast.Node
	SucceedCallExpression *ast.Node
	SucceedArgument       *ast.Node
}

// AnalyzeFlatMapToMap finds piping transformations whose Effect.flatMap
// callback consists solely of an Effect.succeed call.
func AnalyzeFlatMapToMap(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []FlatMapToMapMatch {
	var matches []FlatMapToMapMatch

	flows := tp.PipingFlows(sf, true)
	for _, flow := range flows {
		for _, transformation := range flow.Transformations {
			callee := transformation.Callee
			args := transformation.Args
			if len(args) == 0 && callee != nil && callee.Kind == ast.KindCallExpression {
				call := callee.AsCallExpression()
				if call != nil && call.Arguments != nil {
					callee = call.Expression
					args = call.Arguments.Nodes
				}
			}

			if len(args) == 0 || callee == nil || callee.Kind != ast.KindPropertyAccessExpression {
				continue
			}
			if !tp.IsNodeReferenceToEffectModuleApi(callee, "flatMap") {
				continue
			}

			callback := typeparser.ParseLazyExpression(args[0], false)
			if callback == nil || callback.Expression == nil || callback.Expression.Kind != ast.KindCallExpression {
				continue
			}
			succeedCall := callback.Expression.AsCallExpression()
			if succeedCall == nil || succeedCall.Expression == nil || succeedCall.Arguments == nil || len(succeedCall.Arguments.Nodes) != 1 {
				continue
			}
			if !tp.IsNodeReferenceToEffectModuleApi(succeedCall.Expression, "succeed") {
				continue
			}

			calleeName := callee.AsPropertyAccessExpression().Name()
			if calleeName == nil {
				continue
			}

			matches = append(matches, FlatMapToMapMatch{
				SourceFile:            sf,
				Location:              scanner.GetErrorRangeForNode(sf, callee),
				CalleeNameNode:        calleeName,
				SucceedCallExpression: callback.Expression,
				SucceedArgument:       succeedCall.Arguments.Nodes[0],
			})
		}
	}

	return matches
}
