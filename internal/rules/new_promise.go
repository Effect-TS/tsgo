package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var NewPromise = rule.Rule{
	Name:            "newPromise",
	Group:           "effectNative",
	Description:     "Warns when constructing promises with new Promise instead of using Effect APIs",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_code_constructs_new_Promise_prefer_Effect_APIs_such_as_Effect_async_Effect_promise_or_Effect_tryPromise_instead_of_manual_Promise_construction_effect_newPromise.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		promiseSymbol := ctx.Checker.ResolveName("Promise", nil, ast.SymbolFlagsValue, false)
		if promiseSymbol == nil {
			return nil
		}

		var diags []*ast.Diagnostic
		var walk ast.Visitor
		walk = func(node *ast.Node) bool {
			if node == nil {
				return false
			}

			if node.Kind == ast.KindNewExpression {
				newExpr := node.AsNewExpression()
				if ctx.TypeParser.ResolveToGlobalSymbol(ctx.Checker.GetSymbolAtLocation(newExpr.Expression)) == promiseSymbol {
					diags = append(diags, ctx.NewDiagnostic(
						ctx.SourceFile,
						scanner.GetErrorRangeForNode(ctx.SourceFile, node),
						tsdiag.This_code_constructs_new_Promise_prefer_Effect_APIs_such_as_Effect_async_Effect_promise_or_Effect_tryPromise_instead_of_manual_Promise_construction_effect_newPromise,
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
