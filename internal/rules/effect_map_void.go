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

// EffectMapVoid suggests using Effect.asVoid instead of Effect.map(() => void 0),
// Effect.map(() => undefined), or Effect.map(() => {}).
var EffectMapVoid = rule.Rule{
	Name:            "effectMapVoid",
	Group:           "style",
	Description:     "Suggests using Effect.asVoid instead of Effect.map(() => void 0), Effect.map(() => undefined), or Effect.map(() => {})",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Effect_asVoid_can_be_used_instead_to_discard_the_success_value_effect_effectMapVoid.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeEffectMapVoid(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.Effect_asVoid_can_be_used_instead_to_discard_the_success_value_effect_effectMapVoid, nil)
		}
		return diags
	},
}

// EffectMapVoidMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the effectMapVoid pattern.
type EffectMapVoidMatch struct {
	SourceFile       *ast.SourceFile // The source file where the diagnostic should be reported
	Location         core.TextRange  // The pre-computed error range for this match
	CallNode         *ast.Node       // The Effect.map(...) call expression (replacement target)
	EffectModuleNode *ast.Node       // The Effect module identifier (e.g., "Effect" in Effect.map)
}

// isVoidCallback checks if a callback argument is a "void callback":
// either an empty function (block body with zero statements) or a lazy void expression
// (zero-param function returning void 0, undefined, or parenthesized versions).
func isVoidCallback(node *ast.Node) bool {
	// First try parsing as a lazy expression (thunk=true, requiring zero params)
	lazy := typeparser.ParseLazyExpression(node, true)
	if lazy != nil {
		// ParseLazyExpression succeeded — check if the expression is a void expression
		return isVoidExpression(lazy.Expression)
	}

	// ParseLazyExpression returns nil for empty block bodies (0 statements).
	// Handle that case explicitly: arrow function or function expression with
	// zero parameters, no type parameters, and a block body with zero statements.
	switch node.Kind {
	case ast.KindArrowFunction:
		fn := node.AsArrowFunction()
		if fn.TypeParameters != nil && len(fn.TypeParameters.Nodes) > 0 {
			return false
		}
		if fn.Parameters != nil && len(fn.Parameters.Nodes) > 0 {
			return false
		}
		if fn.Body != nil && fn.Body.Kind == ast.KindBlock {
			block := fn.Body.AsBlock()
			return block.Statements == nil || len(block.Statements.Nodes) == 0
		}
	case ast.KindFunctionExpression:
		fn := node.AsFunctionExpression()
		if fn.TypeParameters != nil && len(fn.TypeParameters.Nodes) > 0 {
			return false
		}
		if fn.Parameters != nil && len(fn.Parameters.Nodes) > 0 {
			return false
		}
		if fn.Body != nil && fn.Body.Kind == ast.KindBlock {
			block := fn.Body.AsBlock()
			return block.Statements == nil || len(block.Statements.Nodes) == 0
		}
	}

	return false
}

// AnalyzeEffectMapVoid finds all Effect.map calls with void callbacks
// that can be replaced with Effect.asVoid.
func AnalyzeEffectMapVoid(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []EffectMapVoidMatch {
	var matches []EffectMapVoidMatch

	var walk ast.Visitor
	walk = func(n *ast.Node) bool {
		if n == nil {
			return false
		}

		if n.Kind == ast.KindCallExpression {
			call := n.AsCallExpression()
			if call.Expression != nil && call.Expression.Kind == ast.KindPropertyAccessExpression {
				if tp.IsNodeReferenceToEffectModuleApi(call.Expression, "map") {
					if call.Arguments != nil && len(call.Arguments.Nodes) >= 1 {
						arg := call.Arguments.Nodes[0]
						if isVoidCallback(arg) {
							propAccess := call.Expression.AsPropertyAccessExpression()
							matches = append(matches, EffectMapVoidMatch{
								SourceFile:       sf,
								Location:         scanner.GetErrorRangeForNode(sf, call.Expression),
								CallNode:         n,
								EffectModuleNode: propAccess.Expression,
							})
						}
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
