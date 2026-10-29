package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

// StrictEffectProvide warns when Effect.provide is called with a Layer argument,
// unless the next step of the flow runs the Effect. This rule is disabled by default.
var StrictEffectProvide = rule.Rule{
	Name:            "strictEffectProvide",
	Group:           "antipattern",
	Description:     "Warns when using Effect.provide with layers outside of application entry points",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Effect_provide_with_a_Layer_should_only_be_used_at_application_entry_points_If_this_is_an_entry_point_you_can_safely_disable_this_diagnostic_Otherwise_using_Effect_provide_may_break_scope_lifetimes_Compose_all_layers_at_your_entry_point_and_provide_them_at_once_effect_strictEffectProvide.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic
		for _, flow := range ctx.TypeParser.PipingFlows(ctx.SourceFile, true).All() {
			transformations := flow.Transformations()
			for index, transformation := range transformations.All() {
				if !isEffectProvideWithLayer(ctx, transformation) {
					continue
				}
				// Providing layers right before running the Effect is what an entry point does
				if index+1 < transformations.Len() && isEntryPointRunner(ctx.TypeParser, transformations.At(index+1).Callee()) {
					continue
				}
				if node := transformationApplicationNode(&transformation); node != nil {
					diags = append(diags, ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(node), tsdiag.Effect_provide_with_a_Layer_should_only_be_used_at_application_entry_points_If_this_is_an_entry_point_you_can_safely_disable_this_diagnostic_Otherwise_using_Effect_provide_may_break_scope_lifetimes_Compose_all_layers_at_your_entry_point_and_provide_them_at_once_effect_strictEffectProvide, nil))
				}
			}
		}
		return diags
	},
}

// isEffectProvideWithLayer checks if a transformation is Effect.provide(...) with a Layer argument.
func isEffectProvideWithLayer(ctx *rule.Context, transformation typeparser.PipingFlowTransformation) bool {
	if transformation.Args().Len() == 0 || !ctx.TypeParser.IsNodeReferenceToEffectModuleApi(transformation.Callee(), "provide") {
		return false
	}
	for _, arg := range transformation.Args().All() {
		if ctx.TypeParser.LayerTypeAtLocation(arg) != nil {
			return true
		}
	}
	return false
}

// entryPointRunnerApis lists the Effect module APIs that run an Effect at an entry point.
var entryPointRunnerApis = []string{"runPromise", "runSync", "runFork", "runPromiseExit", "runSyncExit", "runCallback"}

// isEntryPointRunner reports whether node references an Effect.run* API or a platform runMain.
func isEntryPointRunner(tp *typeparser.TypeParser, node *ast.Node) bool {
	for _, name := range entryPointRunnerApis {
		if tp.IsNodeReferenceToEffectModuleApi(node, name) {
			return true
		}
	}
	return tp.IsNodeReferenceToPlatformRuntimeApi(node, "runMain")
}
