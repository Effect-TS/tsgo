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

// ReturnEffectInGen detects return statements inside Effect generators
// that return an Effect-able type, which would result in nested Effect<Effect<...>>.
var ReturnEffectInGen = rule.Rule{
	Name:            "returnEffectInGen",
	Group:           "antipattern",
	Description:     "Warns when returning an Effect in a generator causes nested Effect<Effect<...>>",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_generator_returns_an_Effect_able_value_directly_which_produces_a_nested_Effect_Effect_If_the_intended_result_is_the_inner_Effect_value_return_yield_Asterisk_represents_that_form_effect_returnEffectInGen.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeReturnEffectInGen(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.This_generator_returns_an_Effect_able_value_directly_which_produces_a_nested_Effect_Effect_If_the_intended_result_is_the_inner_Effect_value_return_yield_Asterisk_represents_that_form_effect_returnEffectInGen, nil)
		}
		return diags
	},
}

// ReturnEffectInGenMatch holds the diagnostic and the return statement node needed
// by both the diagnostic rule and the quick-fix.
type ReturnEffectInGenMatch struct {
	SourceFile *ast.SourceFile // The source file where this match was found
	Location   core.TextRange  // The pre-computed error range for this match
	ReturnNode *ast.Node       // The return statement AST node
}

// AnalyzeReturnEffectInGen finds all return statements inside Effect generators
// that return an Effect-able type, returning matches with both the diagnostic and the return node.
func AnalyzeReturnEffectInGen(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []ReturnEffectInGenMatch {
	var matches []ReturnEffectInGenMatch

	var walk ast.Visitor
	walk = func(n *ast.Node) bool {
		if n == nil {
			return false
		}

		if n.Kind == ast.KindReturnStatement {
			if checkReturnEffectInGenScope(tp, c, sf, n) {
				matches = append(matches, ReturnEffectInGenMatch{
					SourceFile: sf,
					Location:   scanner.GetErrorRangeForNode(sf, n),
					ReturnNode: n,
				})
			}
		}

		n.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())
	return matches
}

// checkReturnEffectInGenScope checks if a return statement inside an Effect generator
// is returning an Effect-able type (which would cause nested Effect<Effect<...>>).
func checkReturnEffectInGenScope(tp *typeparser.TypeParser, _ *checker.Checker, _ *ast.SourceFile, n *ast.Node) bool {
	returnStmt := n.AsReturnStatement()
	if returnStmt == nil || returnStmt.Expression == nil {
		return false
	}

	// return yield* ... is the correct pattern, skip it
	if returnStmt.Expression.Kind == ast.KindYieldExpression {
		return false
	}

	if tp.GetEffectContextFlags(n)&typeparser.EffectContextFlagCanYieldEffect == 0 {
		return false
	}

	t := tp.GetTypeAtLocation(returnStmt.Expression)
	if t == nil {
		return false
	}

	if !tp.StrictIsEffectType(t, returnStmt.Expression) {
		return false
	}

	return true
}
