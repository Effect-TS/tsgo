package rules

import (
	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/checkerutils"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// UnnecessaryEffectGen suggests removing Effect.gen when it contains only a single return statement.
var UnnecessaryEffectGen = rule.Rule{
	Name:        "unnecessaryEffectGen",
	Description:     "Suggests removing Effect.gen when it contains only a single return statement",
	DefaultSeverity: etscore.SeveritySuggestion,
	Codes:       []int32{tsdiag.This_Effect_gen_contains_a_single_return_statement_effect_unnecessaryEffectGen.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeUnnecessaryEffectGen(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.Location, tsdiag.This_Effect_gen_contains_a_single_return_statement_effect_unnecessaryEffectGen, nil)
		}
		return diags
	},
}

// UnnecessaryEffectGenMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the unnecessaryEffectGen pattern.
type UnnecessaryEffectGenMatch struct {
	Location          core.TextRange  // The pre-computed error range for this match
	CallNode          *ast.Node       // The Effect.gen(...) call expression (replacement target)
	YieldedExpression *ast.Node       // The expression being yield*-ed (the replacement value)
	ExplicitReturn    bool            // Whether the statement had an explicit return
	SuccessIsVoid     bool            // Whether the yielded Effect's success type is void-like
	EffectModuleNode  *ast.Node       // The Effect module identifier (for Effect.asVoid wrapping)
}

// AnalyzeUnnecessaryEffectGen finds all Effect.gen calls that contain only a single
// yield* statement and can be replaced with the yielded expression directly.
func AnalyzeUnnecessaryEffectGen(c *checker.Checker, sf *ast.SourceFile) []UnnecessaryEffectGenMatch {
	var matches []UnnecessaryEffectGenMatch

	var walk ast.Visitor
	walk = func(n *ast.Node) bool {
		if n == nil {
			return false
		}

		if n.Kind == ast.KindCallExpression {
			if m := analyzeUnnecessaryEffectGenNode(c, sf, n); m != nil {
				matches = append(matches, *m)
			}
		}

		n.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())
	return matches
}

func analyzeUnnecessaryEffectGenNode(c *checker.Checker, sf *ast.SourceFile, n *ast.Node) *UnnecessaryEffectGenMatch {
	genResult := typeparser.EffectGenCall(c, n)
	if genResult == nil {
		return nil
	}

	// Must have exactly one argument (just the generator, no options object)
	if genResult.Call.Arguments == nil || len(genResult.Call.Arguments.Nodes) != 1 {
		return nil
	}

	body := genResult.Body
	if body == nil || body.Kind != ast.KindBlock {
		return nil
	}

	block := body.AsBlock()
	if block.Statements == nil || len(block.Statements.Nodes) != 1 {
		return nil
	}

	stmt := block.Statements.Nodes[0]

	var expr *ast.Node
	var explicitReturn bool

	switch stmt.Kind {
	case ast.KindReturnStatement:
		expr = stmt.AsReturnStatement().Expression
		if expr == nil {
			return nil
		}
		explicitReturn = true
	case ast.KindExpressionStatement:
		expr = stmt.AsExpressionStatement().Expression
		explicitReturn = false
	default:
		return nil
	}

	// Must be a yield* expression
	if expr.Kind != ast.KindYieldExpression {
		return nil
	}
	yield := expr.AsYieldExpression()
	if yield.AsteriskToken == nil || yield.Expression == nil {
		return nil
	}

	yieldedExpr := yield.Expression

	// Determine if the success type is void-like
	successIsVoid := false
	t := checkerutils.GetTypeAtLocation(c, yieldedExpr)
	if t != nil {
		effect := typeparser.EffectType(c, t, yieldedExpr)
		if effect != nil && effect.A != nil {
			successIsVoid = effect.A.Flags()&checker.TypeFlagsVoidLike != 0
		}
	}

	return &UnnecessaryEffectGenMatch{
		Location:          scanner.GetErrorRangeForNode(sf, n),
		CallNode:          n,
		YieldedExpression: yieldedExpr,
		ExplicitReturn:    explicitReturn,
		SuccessIsVoid:     successIsVoid,
		EffectModuleNode:  genResult.EffectModule,
	}
}

