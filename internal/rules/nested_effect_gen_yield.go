package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// NestedEffectGenYield warns when yield* targets a bare nested Effect.gen inside
// an existing Effect generator context, since the inner generator can usually be
// inlined directly into the surrounding Effect generator.
var NestedEffectGenYield = rule.Rule{
	Name:            "nestedEffectGenYield",
	Group:           "style",
	Description:     "Warns when yielding a nested bare Effect.gen inside an existing Effect generator context",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.This_yield_Asterisk_is_applied_to_a_nested_Effect_gen_that_can_be_inlined_in_the_parent_Effect_generator_context_effect_nestedEffectGenYield.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeNestedEffectGenYield(ctx.TypeParser, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				ctx.SourceFile,
				match.Location,
				tsdiag.This_yield_Asterisk_is_applied_to_a_nested_Effect_gen_that_can_be_inlined_in_the_parent_Effect_generator_context_effect_nestedEffectGenYield,
				nil,
			)
		}
		return diags
	},
}

type NestedEffectGenYieldMatch struct {
	Location core.TextRange
}

func AnalyzeNestedEffectGenYield(tp *typeparser.TypeParser, sf *ast.SourceFile) []NestedEffectGenYieldMatch {
	var matches []NestedEffectGenYieldMatch

	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}

		if node.Kind == ast.KindYieldExpression {
			yieldExpr := node.AsYieldExpression()
			if yieldExpr != nil && yieldExpr.AsteriskToken != nil && yieldExpr.Expression != nil {
				if tp.GetEffectContextFlags(node)&typeparser.EffectContextFlagCanYieldEffect != 0 {
					if tp.EffectGenCall(yieldExpr.Expression) != nil {
						matches = append(matches, NestedEffectGenYieldMatch{
							Location: scanner.GetErrorRangeForNode(sf, yieldExpr.Expression),
						})
					}
				}
			}
		}

		node.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())
	return matches
}
