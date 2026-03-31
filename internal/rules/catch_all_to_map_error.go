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

// CatchAllToMapError suggests using Effect.mapError instead of Effect.catch + Effect.fail.
var CatchAllToMapError = rule.Rule{
	Name:            "catchAllToMapError",
	Group:           "style",
	Description:     "Suggests using Effect.mapError instead of Effect.catch + Effect.fail",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.You_can_use_Effect_mapError_instead_of_Effect_catch_Effect_fail_to_transform_the_error_type_effect_catchAllToMapError.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeCatchAllToMapError(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.You_can_use_Effect_mapError_instead_of_Effect_catch_Effect_fail_to_transform_the_error_type_effect_catchAllToMapError, nil)
		}
		return diags
	},
}

// CatchAllToMapErrorMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the catchAllToMapError pattern.
type CatchAllToMapErrorMatch struct {
	SourceFile         *ast.SourceFile // The source file of the match
	Location           core.TextRange  // The pre-computed error range for this match
	Callee             *ast.Node       // The Effect.catch callee node (for diagnostic location)
	CalleeNameNode     *ast.Node       // The "catch" name node within the PropertyAccessExpression (for text replacement)
	FailCallExpression *ast.Node       // The Effect.fail(arg) call expression node (for replacement range)
	FailArgument       *ast.Node       // The first argument to Effect.fail (the replacement text)
}

// AnalyzeCatchAllToMapError finds all Effect.catch callbacks that simply wrap the
// error with Effect.fail, which can be simplified to Effect.mapError.
func AnalyzeCatchAllToMapError(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []CatchAllToMapErrorMatch {
	var matches []CatchAllToMapErrorMatch

	flows := tp.PipingFlows(sf, true)
	for _, flow := range flows {
		for _, transformation := range flow.Transformations {
			if !tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "catch") &&
				!tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "catchAll") {
				continue
			}

			if len(transformation.Args) < 1 {
				continue
			}
			callback := transformation.Args[0]

			lazy := typeparser.ParseLazyExpression(callback, false)
			if lazy == nil {
				continue
			}

			expr := lazy.Expression
			if expr == nil || expr.Kind != ast.KindCallExpression {
				continue
			}
			call := expr.AsCallExpression()
			if call == nil || call.Expression == nil {
				continue
			}
			if call.Arguments == nil || len(call.Arguments.Nodes) < 1 {
				continue
			}

			if !tp.IsNodeReferenceToEffectModuleApi(call.Expression, "fail") {
				continue
			}

			// Extract the "catch" name node from the PropertyAccessExpression callee
			var calleeNameNode *ast.Node
			callee := transformation.Callee
			if callee.Kind == ast.KindPropertyAccessExpression {
				prop := callee.AsPropertyAccessExpression()
				if prop != nil && prop.Name() != nil {
					calleeNameNode = prop.Name()
				}
			}

			matches = append(matches, CatchAllToMapErrorMatch{
				SourceFile:         sf,
				Location:           scanner.GetErrorRangeForNode(sf, transformation.Callee),
				Callee:             transformation.Callee,
				CalleeNameNode:     calleeNameNode,
				FailCallExpression: expr,
				FailArgument:       call.Arguments.Nodes[0],
			})
		}
	}

	return matches
}
