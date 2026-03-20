package rules

import (
	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// MissingStarInYieldEffectGen detects bare yield (without *) inside Effect generator scopes.
var MissingStarInYieldEffectGen = rule.Rule{
	Name:            "missingStarInYieldEffectGen",
	Group:           "correctness",
	Description:     "Detects bare yield (without *) inside Effect generator scopes",
	DefaultSeverity: etscore.SeverityError,
	SupportedEffect: []string{"v3", "v4"},
	Codes:       []int32{tsdiag.Inside_this_Effect_generator_effect_missingStarInYieldEffectGen.Code(), tsdiag.When_yielding_Effects_inside_Effect_gen_you_should_use_yield_Asterisk_instead_of_yield_effect_missingStarInYieldEffectGen.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeMissingStarInYieldEffectGen(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			relatedInfo := ctx.NewDiagnostic(m.SourceFile, ctx.GetErrorRange(m.GenFnNode), tsdiag.Inside_this_Effect_generator_effect_missingStarInYieldEffectGen, nil)
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.When_yielding_Effects_inside_Effect_gen_you_should_use_yield_Asterisk_instead_of_yield_effect_missingStarInYieldEffectGen, []*ast.Diagnostic{relatedInfo})
		}
		return diags
	},
}

// MissingStarInYieldEffectGenMatch holds the AST node needed by both the diagnostic rule
// and the quick-fix for the missingStarInYieldEffectGen pattern.
type MissingStarInYieldEffectGenMatch struct {
	SourceFile *ast.SourceFile // The source file where the diagnostic should be reported
	Location  core.TextRange  // The pre-computed error range for this match
	YieldNode  *ast.Node       // The yield expression node (for fix range)
	GenFnNode  *ast.Node       // The generator function node (for related info)
}

// AnalyzeMissingStarInYieldEffectGen finds all yield expressions inside Effect generators
// that are missing the asterisk (yield instead of yield*).
func AnalyzeMissingStarInYieldEffectGen(c *checker.Checker, sf *ast.SourceFile) []MissingStarInYieldEffectGenMatch {
	var matches []MissingStarInYieldEffectGenMatch

	var walk ast.Visitor
	walk = func(n *ast.Node) bool {
		if n == nil {
			return false
		}

		if n.Kind == ast.KindYieldExpression {
			yield := n.AsYieldExpression()
			if yield != nil && yield.Expression != nil && yield.AsteriskToken == nil {
				scopes := typeparser.FindEnclosingScopes(c, n)
				if scopes.ScopeNode != nil &&
					(scopes.ScopeKind == typeparser.ScopeKindEffectGen || scopes.ScopeKind == typeparser.ScopeKindEffectFn) {
					genFn := scopes.EffectGeneratorFunction()
					if genFn != nil && scopes.ScopeNode == genFn.AsNode() {
						matches = append(matches, MissingStarInYieldEffectGenMatch{
							SourceFile: sf,
							Location:  scanner.GetErrorRangeForNode(sf, n),
							YieldNode: n,
							GenFnNode:  genFn.AsNode(),
						})
					}
				}
			}
		}

		n.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())

	return matches
}
