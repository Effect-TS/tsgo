package rules

import (
	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/checkerutils"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
)

// StrictEffectProvide warns when Effect.provide is called with a Layer argument.
// This rule is disabled by default.
var StrictEffectProvide = rule.Rule{
	Name:            "strictEffectProvide",
	Description:     "Warns when using Effect.provide with layers outside of application entry points",
	DefaultSeverity: etscore.SeverityOff,
	Codes:           []int32{tsdiag.Effect_provide_with_a_Layer_should_only_be_used_at_application_entry_points_If_this_is_an_entry_point_you_can_safely_disable_this_diagnostic_Otherwise_using_Effect_provide_may_break_scope_lifetimes_Compose_all_layers_at_your_entry_point_and_provide_them_at_once_effect_strictEffectProvide.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic

		// Stack-based traversal
		nodeToVisit := make([]*ast.Node, 0)
		pushChild := func(child *ast.Node) bool {
			nodeToVisit = append(nodeToVisit, child)
			return false
		}
		ctx.SourceFile.AsNode().ForEachChild(pushChild)

		for len(nodeToVisit) > 0 {
			node := nodeToVisit[len(nodeToVisit)-1]
			nodeToVisit = nodeToVisit[:len(nodeToVisit)-1]

			if node.Kind == ast.KindCallExpression {
				if diag := checkEffectProvideWithLayer(ctx, node); diag != nil {
					diags = append(diags, diag)
				}
			}

			// Enqueue children
			node.ForEachChild(pushChild)
		}

		return diags
	},
}

// checkEffectProvideWithLayer checks if a call expression is Effect.provide(...) with a Layer argument.
func checkEffectProvideWithLayer(ctx *rule.Context, node *ast.Node) *ast.Diagnostic {
	if node.Kind != ast.KindCallExpression {
		return nil
	}
	call := node.AsCallExpression()
	args := call.Arguments
	if args == nil || len(args.Nodes) == 0 {
		return nil
	}

	// Check if the expression references Effect.provide
	if !typeparser.IsNodeReferenceToEffectModuleApi(ctx.Checker, call.Expression, "provide") {
		return nil
	}

	// Check if any argument is a Layer type
	for _, arg := range args.Nodes {
		argType := checkerutils.GetTypeAtLocation(ctx.Checker, arg)
		if argType == nil {
			continue
		}
		if typeparser.LayerType(ctx.Checker, argType, arg) != nil {
			// Found a Layer argument — emit diagnostic on the call expression
			return ctx.NewDiagnostic(ctx.GetErrorRange(node), tsdiag.Effect_provide_with_a_Layer_should_only_be_used_at_application_entry_points_If_this_is_an_entry_point_you_can_safely_disable_this_diagnostic_Otherwise_using_Effect_provide_may_break_scope_lifetimes_Compose_all_layers_at_your_entry_point_and_provide_them_at_once_effect_strictEffectProvide, nil)
		}
	}

	return nil
}
