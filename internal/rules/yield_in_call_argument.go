package rules

import (
	"slices"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// YieldInCallArgument suggests yielding into a const before a call instead of
// passing a yield* directly as a call or new argument in Effect generators, so
// running the effect and building the call stay separate steps.
var YieldInCallArgument = rule.Rule{
	Name:            "yieldInCallArgument",
	Group:           "style",
	Description:     "Suggests yielding into a const before a call instead of passing a yield* directly as a call or new argument in Effect generators",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.This_yield_Asterisk_is_used_as_a_call_argument_yield_it_into_a_const_first_or_map_the_effect_so_running_the_effect_and_building_the_call_stay_separate_steps_effect_yieldInCallArgument.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeYieldInCallArgument(ctx.TypeParser, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				ctx.SourceFile,
				match.Location,
				tsdiag.This_yield_Asterisk_is_used_as_a_call_argument_yield_it_into_a_const_first_or_map_the_effect_so_running_the_effect_and_building_the_call_stay_separate_steps_effect_yieldInCallArgument,
				nil,
			)
		}
		return diags
	},
}

type YieldInCallArgumentMatch struct {
	Location core.TextRange
	// Yield is the yield* expression passed as an argument.
	Yield *ast.Node
	// Argument is Yield with its enclosing parentheses, the node the fix replaces.
	Argument *ast.Node
}

func AnalyzeYieldInCallArgument(tp *typeparser.TypeParser, sf *ast.SourceFile) []YieldInCallArgumentMatch {
	var matches []YieldInCallArgumentMatch

	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}

		if node.Kind == ast.KindYieldExpression {
			yieldExpr := node.AsYieldExpression()
			if yieldExpr.AsteriskToken != nil && yieldExpr.Expression != nil {
				if arg := yieldInCallArgumentArgument(node); arg != nil && tp.GetEffectContextFlags(node)&typeparser.EffectContextFlagCanYieldEffect != 0 {
					matches = append(matches, YieldInCallArgumentMatch{
						Location: scanner.GetErrorRangeForNode(sf, node),
						Yield:    node,
						Argument: arg,
					})
				}
			}
		}

		//ets:ast-walk finds yield* call arguments; new expressions and multi-argument calls to a non-dual callee produce no flow step
		node.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())
	return matches
}

// yieldInCallArgumentArgument returns the argument of a call or new expression
// that is node, or node in parentheses, or nil when node is not an argument.
func yieldInCallArgumentArgument(node *ast.Node) *ast.Node {
	call := ast.WalkUpParenthesizedExpressions(node.Parent)
	if call == nil || !ast.IsCallOrNewExpression(call) {
		return nil
	}
	arguments := call.Arguments()
	if i := slices.IndexFunc(arguments, func(argument *ast.Node) bool { return ast.SkipParentheses(argument) == node }); i >= 0 {
		return arguments[i]
	}
	return nil
}
