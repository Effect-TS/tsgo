package rules

import (
	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
)

// EffectGenUsesAdapter warns when using the deprecated adapter parameter in Effect.gen.
var EffectGenUsesAdapter = rule.Rule{
	Name:            "effectGenUsesAdapter",
	Description:     "Warns when using the deprecated adapter parameter in Effect.gen",
	DefaultSeverity: etscore.SeverityWarning,
	Codes:       []int32{tsdiag.The_adapter_of_Effect_gen_is_not_required_anymore_it_is_now_just_an_alias_of_pipe_effect_effectGenUsesAdapter.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic

		var walk ast.Visitor
		walk = func(n *ast.Node) bool {
			if n == nil {
				return false
			}

			if n.Kind == ast.KindCallExpression {
				if diag := checkEffectGenUsesAdapter(ctx, n); diag != nil {
					diags = append(diags, diag)
				}
			}

			n.ForEachChild(walk)
			return false
		}

		walk(ctx.SourceFile.AsNode())

		return diags
	},
}

func checkEffectGenUsesAdapter(ctx *rule.Context, n *ast.Node) *ast.Diagnostic {
	genResult := typeparser.EffectGenCall(ctx.Checker, n)
	if genResult == nil {
		return nil
	}

	if genResult.GeneratorFunction.Parameters == nil || len(genResult.GeneratorFunction.Parameters.Nodes) == 0 {
		return nil
	}

	adapterParam := genResult.GeneratorFunction.Parameters.Nodes[0]
	return ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(adapterParam), tsdiag.The_adapter_of_Effect_gen_is_not_required_anymore_it_is_now_just_an_alias_of_pipe_effect_effectGenUsesAdapter, nil)
}
