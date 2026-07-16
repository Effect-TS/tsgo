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

// SyncToSucceed suggests using Effect.succeed when an Effect.sync thunk returns
// a value that is already constant at Effect construction time.
var SyncToSucceed = rule.Rule{
	Name:            "syncToSucceed",
	Group:           "style",
	Description:     "Suggests using Effect.succeed instead of Effect.sync when the thunk returns a constant value",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_succeed_expresses_this_constant_value_more_directly_than_Effect_sync_effect_syncToSucceed.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeSyncToSucceed(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_succeed_expresses_this_constant_value_more_directly_than_Effect_sync_effect_syncToSucceed,
				nil,
			)
		}
		return diags
	},
}

// SyncToSucceedMatch holds the nodes needed by the diagnostic and quick fix.
type SyncToSucceedMatch struct {
	SourceFile    *ast.SourceFile
	Location      core.TextRange
	CalleeName    *ast.Node
	Thunk         *ast.Node
	ConstantValue *ast.Node
}

// AnalyzeSyncToSucceed finds Effect.sync thunks whose result is already stable
// when the Effect is constructed.
func AnalyzeSyncToSucceed(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []SyncToSucceedMatch {
	var matches []SyncToSucceedMatch
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}

		if node.Kind == ast.KindCallExpression {
			call := node.AsCallExpression()
			if call != nil && call.Expression != nil && call.Expression.Kind == ast.KindPropertyAccessExpression &&
				tp.IsNodeReferenceToEffectModuleApi(call.Expression, "sync") && call.Arguments != nil && len(call.Arguments.Nodes) == 1 {
				lazy := typeparser.ParseLazyExpression(call.Arguments.Nodes[0], true)
				if lazy != nil && isSynchronousNonGeneratorFunction(lazy.Node) && tp.IsExpressionValueStableAtLocation(lazy.Expression, node) {
					calleeName := call.Expression.AsPropertyAccessExpression().Name()
					if calleeName != nil {
						matches = append(matches, SyncToSucceedMatch{
							SourceFile:    sf,
							Location:      scanner.GetErrorRangeForNode(sf, call.Expression),
							CalleeName:    calleeName,
							Thunk:         lazy.Node,
							ConstantValue: lazy.Expression,
						})
					}
				}
			}
		}

		node.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())
	return matches
}

func isSynchronousNonGeneratorFunction(node *ast.Node) bool {
	if node == nil || ast.GetCombinedModifierFlags(node)&ast.ModifierFlagsAsync != 0 {
		return false
	}
	if node.Kind == ast.KindFunctionExpression {
		fn := node.AsFunctionExpression()
		return fn != nil && fn.AsteriskToken == nil
	}
	return node.Kind == ast.KindArrowFunction
}
