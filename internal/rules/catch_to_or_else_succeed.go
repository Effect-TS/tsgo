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

// CatchToOrElseSucceed suggests using Effect.orElseSucceed instead of Effect.catch + Effect.succeed.
var CatchToOrElseSucceed = rule.Rule{
	Name:            "catchToOrElseSucceed",
	Group:           "style",
	Description:     "Suggests using Effect.orElseSucceed instead of Effect.catch + Effect.succeed",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Effect_orElseSucceed_expresses_the_same_recovery_more_directly_than_Effect_0_followed_by_Effect_succeed_effect_catchToOrElseSucceed.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeCatchToOrElseSucceed(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.Effect_orElseSucceed_expresses_the_same_recovery_more_directly_than_Effect_0_followed_by_Effect_succeed_effect_catchToOrElseSucceed, nil, m.CatchMethodName)
		}
		return diags
	},
}

// CatchToOrElseSucceedMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the catchToOrElseSucceed pattern.
type CatchToOrElseSucceedMatch struct {
	SourceFile            *ast.SourceFile
	Location              core.TextRange
	CalleeNameNode        *ast.Node
	CatchMethodName       string
	SucceedCallExpression *ast.Node
	SucceedArgument       *ast.Node
}

// AnalyzeCatchToOrElseSucceed finds Effect.catch callbacks that ignore the error
// and return Effect.succeed, which can be simplified to Effect.orElseSucceed.
func AnalyzeCatchToOrElseSucceed(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []CatchToOrElseSucceedMatch {
	var matches []CatchToOrElseSucceedMatch

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

			lazy := typeparser.ParseLazyExpression(transformation.Args[0], true)
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

			if !tp.IsNodeReferenceToEffectModuleApi(call.Expression, "succeed") {
				continue
			}

			var calleeNameNode *ast.Node
			catchMethodName := "catch"
			if transformation.Callee.Kind == ast.KindPropertyAccessExpression {
				prop := transformation.Callee.AsPropertyAccessExpression()
				if prop != nil && prop.Name() != nil {
					calleeNameNode = prop.Name()
					catchMethodName = prop.Name().Text()
				}
			}

			if call.Arguments == nil || len(call.Arguments.Nodes) < 1 {
				continue
			}

			matches = append(matches, CatchToOrElseSucceedMatch{
				SourceFile:            sf,
				Location:              scanner.GetErrorRangeForNode(sf, transformation.Callee),
				CalleeNameNode:        calleeNameNode,
				CatchMethodName:       catchMethodName,
				SucceedCallExpression: expr,
				SucceedArgument:       call.Arguments.Nodes[0],
			})
		}
	}

	return matches
}
