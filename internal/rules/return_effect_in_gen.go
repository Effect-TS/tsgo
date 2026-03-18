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

// ReturnEffectInGen detects return statements inside Effect generators
// that return an Effect-able type, which would result in nested Effect<Effect<...>>.
var ReturnEffectInGen = rule.Rule{
	Name:        "returnEffectInGen",
	Description:     "Warns when returning an Effect in a generator causes nested Effect<Effect<...>>",
	DefaultSeverity: etscore.SeveritySuggestion,
	Codes:       []int32{tsdiag.You_are_returning_an_Effect_able_type_inside_a_generator_function_and_will_result_in_nested_Effect_Effect_Maybe_you_wanted_to_return_yield_Asterisk_instead_Nested_Effect_able_types_may_be_intended_if_you_plan_to_later_manually_flatten_or_unwrap_this_Effect_if_so_you_can_safely_disable_this_diagnostic_for_this_line_through_quickfixes_effect_returnEffectInGen.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeReturnEffectInGen(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.You_are_returning_an_Effect_able_type_inside_a_generator_function_and_will_result_in_nested_Effect_Effect_Maybe_you_wanted_to_return_yield_Asterisk_instead_Nested_Effect_able_types_may_be_intended_if_you_plan_to_later_manually_flatten_or_unwrap_this_Effect_if_so_you_can_safely_disable_this_diagnostic_for_this_line_through_quickfixes_effect_returnEffectInGen, nil)
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
func AnalyzeReturnEffectInGen(c *checker.Checker, sf *ast.SourceFile) []ReturnEffectInGenMatch {
	var matches []ReturnEffectInGenMatch

	var walk ast.Visitor
	walk = func(n *ast.Node) bool {
		if n == nil {
			return false
		}

		if n.Kind == ast.KindReturnStatement {
			if checkReturnEffectInGenScope(c, sf, n) {
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
func checkReturnEffectInGenScope(c *checker.Checker, sf *ast.SourceFile, n *ast.Node) bool {
	returnStmt := n.AsReturnStatement()
	if returnStmt == nil || returnStmt.Expression == nil {
		return false
	}

	// return yield* ... is the correct pattern, skip it
	if returnStmt.Expression.Kind == ast.KindYieldExpression {
		return false
	}

	scopes := typeparser.FindEnclosingScopes(c, n)
	if scopes.ScopeKind != typeparser.ScopeKindEffectGen && scopes.ScopeKind != typeparser.ScopeKindEffectFn {
		return false
	}

	genFn := scopes.EffectGeneratorFunction()
	if genFn == nil {
		return false
	}

	// The nearest function scope must be the generator itself,
	// not a nested callback, arrow function, or getter.
	if scopes.ScopeNode != genFn.AsNode() {
		return false
	}

	t := checkerutils.GetTypeAtLocation(c, returnStmt.Expression)
	if t == nil {
		return false
	}

	if !typeparser.StrictIsEffectType(c, t, returnStmt.Expression) {
		return false
	}

	return true
}

