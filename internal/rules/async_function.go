package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
)

var AsyncFunction = rule.Rule{
	Name:            "asyncFunction",
	Group:           "effectNative",
	Description:     "Warns when declaring async functions and suggests using Effect values and Effect.gen for async control flow",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_code_declares_an_async_function_consider_representing_this_async_control_flow_with_Effect_values_and_Effect_gen_effect_asyncFunction.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic

		var walk ast.Visitor
		walk = func(node *ast.Node) bool {
			if node == nil {
				return false
			}

			switch node.Kind {
			case ast.KindFunctionDeclaration, ast.KindFunctionExpression, ast.KindArrowFunction, ast.KindMethodDeclaration:
				if ast.GetCombinedModifierFlags(node)&ast.ModifierFlagsAsync != 0 {
					diags = append(diags, ctx.NewDiagnostic(
						ctx.SourceFile,
						ctx.GetErrorRange(node),
						tsdiag.This_code_declares_an_async_function_consider_representing_this_async_control_flow_with_Effect_values_and_Effect_gen_effect_asyncFunction,
						nil,
					))
				}
			}

			node.ForEachChild(walk)
			return false
		}

		walk(ctx.SourceFile.AsNode())
		return diags
	},
}
