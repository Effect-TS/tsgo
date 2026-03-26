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

// EffectFnIife detects Effect.fn or Effect.fnUntraced calls that are immediately invoked (IIFE pattern).
var EffectFnIife = rule.Rule{
	Name:            "effectFnIife",
	Group:           "antipattern",
	Description:     "Effect.fn or Effect.fnUntraced is called as an IIFE; use Effect.gen instead",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.X_0_1_returns_a_reusable_function_that_can_take_arguments_but_here_it_s_called_immediately_Use_Effect_gen_instead_2_effect_effectFnIife.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeEffectFnIife(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			result := m.Result
			effectModuleName := "Effect"
			if result.EffectModule != nil && result.EffectModule.Kind == ast.KindIdentifier {
				effectModuleName = scanner.GetTextOfNode(result.EffectModule)
			}
			withSpanHint := ""
			if result.TraceExpression != nil {
				traceText := ctx.SourceFile.Text()[result.TraceExpression.Pos():result.TraceExpression.End()]
				withSpanHint = " with Effect.withSpan(" + traceText + ") piped in the end to maintain tracing spans"
			}
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.X_0_1_returns_a_reusable_function_that_can_take_arguments_but_here_it_s_called_immediately_Use_Effect_gen_instead_2_effect_effectFnIife, nil, effectModuleName, result.Variant, withSpanHint)
		}
		return diags
	},
}

// EffectFnIifeMatch holds the parsed result needed by both the diagnostic rule
// and the quick-fix for the effectFnIife pattern.
type EffectFnIifeMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	Result     *typeparser.EffectFnIifeResult
}

// AnalyzeEffectFnIife finds all Effect.fn or Effect.fnUntraced calls that are
// immediately invoked (IIFE pattern) in the given source file.
func AnalyzeEffectFnIife(c *checker.Checker, sf *ast.SourceFile) []EffectFnIifeMatch {
	var matches []EffectFnIifeMatch

	var walk ast.Visitor
	walk = func(n *ast.Node) bool {
		if n == nil {
			return false
		}

		if result := typeparser.ParseEffectFnIife(c, n); result != nil {
			matches = append(matches, EffectFnIifeMatch{
				SourceFile: sf,
				Location:   scanner.GetErrorRangeForNode(sf, result.OuterCall.AsNode()),
				Result:     result,
			})
		}

		n.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())

	return matches
}
