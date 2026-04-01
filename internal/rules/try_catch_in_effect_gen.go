// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
)

// TryCatchInEffectGen detects try/catch statements inside Effect generators
// and suggests using Effect's error handling mechanisms instead.
var TryCatchInEffectGen = rule.Rule{
	Name:            "tryCatchInEffectGen",
	Group:           "antipattern",
	Description:     "Discourages try/catch in Effect generators in favor of Effect error handling",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_Effect_generator_contains_try_Slashcatch_in_this_context_error_handling_is_expressed_with_Effect_APIs_such_as_Effect_try_Effect_tryPromise_Effect_catch_Effect_catchTag_effect_tryCatchInEffectGen.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic

		var walk ast.Visitor
		walk = func(n *ast.Node) bool {
			if n == nil {
				return false
			}

			if n.Kind == ast.KindTryStatement {
				tryStmt := n.AsTryStatement()
				if tryStmt != nil && tryStmt.CatchClause != nil {
					if diag := checkTryCatchScope(ctx, n); diag != nil {
						diags = append(diags, diag)
					}
				}
			}

			n.ForEachChild(walk)
			return false
		}

		walk(ctx.SourceFile.AsNode())
		return diags
	},
}

// checkTryCatchScope checks if the try statement is directly inside an Effect
// generator scope using FindEnclosingScopes.
func checkTryCatchScope(ctx *rule.Context, tryNode *ast.Node) *ast.Diagnostic {
	if ctx.TypeParser.GetEffectContextFlags(tryNode)&typeparser.EffectContextFlagCanYieldEffect != 0 {
		return ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(tryNode), tsdiag.This_Effect_generator_contains_try_Slashcatch_in_this_context_error_handling_is_expressed_with_Effect_APIs_such_as_Effect_try_Effect_tryPromise_Effect_catch_Effect_catchTag_effect_tryCatchInEffectGen, nil)
	}
	return nil
}
