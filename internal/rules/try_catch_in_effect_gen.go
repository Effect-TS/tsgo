// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
)

// TryCatchInEffectGen detects try/catch statements inside Effect generators
// and suggests using Effect's error handling mechanisms instead.
var TryCatchInEffectGen = rule.Rule{
	Name:        "tryCatchInEffectGen",
	Description:     "Discourages try/catch in Effect generators in favor of Effect error handling",
	DefaultSeverity: etscore.SeveritySuggestion,
	Codes:       []int32{tsdiag.Avoid_using_try_Slashcatch_inside_Effect_generators_Use_Effect_s_error_handling_mechanisms_instead_e_g_Effect_try_Effect_tryPromise_Effect_catch_Effect_catchTag_effect_tryCatchInEffectGen.Code()},
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
	scopes := typeparser.FindEnclosingScopes(ctx.Checker, tryNode)
	if scopes.ScopeKind == typeparser.ScopeKindEffectGen || scopes.ScopeKind == typeparser.ScopeKindEffectFn {
		genFn := scopes.EffectGeneratorFunction()
		if genFn != nil && scopes.ScopeNode == genFn.AsNode() {
			return ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(tryNode), tsdiag.Avoid_using_try_Slashcatch_inside_Effect_generators_Use_Effect_s_error_handling_mechanisms_instead_e_g_Effect_try_Effect_tryPromise_Effect_catch_Effect_catchTag_effect_tryCatchInEffectGen, nil)
		}
	}
	return nil
}
