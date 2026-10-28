package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

// StrictEffectProvide warns when Effect.provide is called with a Layer argument,
// unless the provided Effect goes straight into a runner. This rule is disabled by default.
var StrictEffectProvide = rule.Rule{
	Name:            "strictEffectProvide",
	Group:           "antipattern",
	Description:     "Warns when using Effect.provide with layers outside of application entry points",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
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
	if !ctx.TypeParser.IsNodeReferenceToEffectModuleApi(call.Expression, "provide") {
		return nil
	}

	// Check if any argument is a Layer type
	for _, arg := range args.Nodes {
		if ctx.TypeParser.NodeCannotBeEffectLibraryType(arg) {
			continue
		}
		argType := ctx.TypeParser.GetTypeAtLocation(arg)
		if argType == nil {
			continue
		}
		if ctx.TypeParser.LayerType(argType) != nil {
			// Providing layers right before running the Effect is what an entry point does
			if isEffectProvidePassedToRunner(ctx.TypeParser, node) {
				return nil
			}
			// Found a Layer argument — emit diagnostic on the call expression
			return ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(node), tsdiag.Effect_provide_with_a_Layer_should_only_be_used_at_application_entry_points_If_this_is_an_entry_point_you_can_safely_disable_this_diagnostic_Otherwise_using_Effect_provide_may_break_scope_lifetimes_Compose_all_layers_at_your_entry_point_and_provide_them_at_once_effect_strictEffectProvide, nil)
		}
	}

	return nil
}

// isEffectProvidePassedToRunner reports whether the Effect returned by a provide call goes straight
// into a runner: the provide call, or the pipe ending with it, is the runner's argument, or the
// runner is the next step of the same pipe.
func isEffectProvidePassedToRunner(tp *typeparser.TypeParser, node *ast.Node) bool {
	expr := skipOuterParentheses(node)
	// A data-last provide applied to its Effect, as in Effect.provide(layer)(program)
	if expr.Parent != nil && expr.Parent.Kind == ast.KindCallExpression && expr.Parent.AsCallExpression().Expression == expr {
		expr = skipOuterParentheses(expr.Parent)
	}
	if expr.Parent != nil && expr.Parent.Kind == ast.KindCallExpression {
		if pipe := tp.ParsePipeCall(expr.Parent); pipe != nil {
			args := pipe.Args()
			for i := range args.Len() {
				if args.At(i) != expr {
					continue
				}
				if i+1 < args.Len() {
					return isEffectRunnerPipeStep(tp, args.At(i+1))
				}
				expr = skipOuterParentheses(pipe.Node().AsNode())
				break
			}
		}
	}

	parent := expr.Parent
	if parent == nil || parent.Kind != ast.KindCallExpression {
		return false
	}
	call := parent.AsCallExpression()
	if call.Arguments == nil || len(call.Arguments.Nodes) == 0 || call.Arguments.Nodes[0] != expr {
		return false
	}
	return isEffectRunner(tp, call.Expression)
}

// isEffectRunnerPipeStep reports whether a pipe step runs the Effect: a runner, or a platform
// runMain applied to its options.
func isEffectRunnerPipeStep(tp *typeparser.TypeParser, step *ast.Node) bool {
	step = ast.SkipParentheses(step)
	if isEffectRunner(tp, step) {
		return true
	}
	return step.Kind == ast.KindCallExpression &&
		tp.IsNodeReferenceToPlatformRuntimeApi(step.AsCallExpression().Expression, platformRunMainApi)
}

// isEffectRunner reports whether node references one of the Effect.run* APIs or a platform runMain.
func isEffectRunner(tp *typeparser.TypeParser, node *ast.Node) bool {
	return matchRunEffectApi(tp, nil, node) != "" || tp.IsNodeReferenceToPlatformRuntimeApi(node, platformRunMainApi)
}

// skipOuterParentheses returns the outermost parenthesized expression wrapping node, or node itself.
func skipOuterParentheses(node *ast.Node) *ast.Node {
	for node.Parent != nil && node.Parent.Kind == ast.KindParenthesizedExpression {
		node = node.Parent
	}
	return node
}
