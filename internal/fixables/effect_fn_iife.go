package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var EffectFnIifeFix = fixable.Fixable{
	Name:        "effectFnIife",
	Description: "Convert Effect.fn IIFE to Effect.gen",
	ErrorCodes:  []int32{tsdiag.X_0_1_returns_a_reusable_function_that_can_take_arguments_but_here_it_s_called_immediately_Use_Effect_gen_instead_2_effect_effectFnIife.Code()},
	FixIDs:      []string{"effectFnIife_toEffectGen"},
	Run:         runEffectFnIifeFix,
}

func runEffectFnIifeFix(ctx *fixable.Context) []ls.CodeAction {

	c := ctx.Checker

	sf := ctx.SourceFile

	// Use shared analysis to find all IIFE matches, then find the one at the diagnostic span
	matches := rules.AnalyzeEffectFnIife(c, sf)
	var result *typeparser.EffectFnIifeResult
	for _, match := range matches {
		diagRange := match.Location
		if diagRange.Intersects(ctx.Span) || ctx.Span.ContainedBy(diagRange) {
			result = match.Result
			break
		}
	}
	if result == nil {
		return nil
	}

	// Only produce a fix for generator functions with zero parameters
	if result.GeneratorFunction == nil {
		return nil
	}
	genFn := result.GeneratorFunction
	if genFn.Parameters != nil && len(genFn.Parameters.Nodes) > 0 {
		return nil
	}

	if action := ctx.NewFixAction(fixable.FixAction{
		Description: "Convert to Effect.gen",
		Run: func(tracker *change.Tracker) {
			// Deep-clone the generator function to produce a synthesized copy
			clonedGenFn := tracker.DeepCloneNode(genFn.AsNode())

			// Clone the Effect module identifier (or create a fallback)
			var effectModuleId *ast.Node
			if result.EffectModule != nil && result.EffectModule.Kind == ast.KindIdentifier {
				effectModuleId = tracker.DeepCloneNode(result.EffectModule)
			} else {
				effectModuleId = tracker.NewIdentifier("Effect")
			}

			// Build Effect.gen property access
			effectGenAccess := tracker.NewPropertyAccessExpression(effectModuleId, nil, tracker.NewIdentifier("gen"), ast.NodeFlagsNone)

			// Build Effect.gen(genFn) call expression
			genCallNode := tracker.NewCallExpression(effectGenAccess, nil, nil, tracker.NewNodeList([]*ast.Node{clonedGenFn}), ast.NodeFlagsNone)

			// If pipe arguments exist, wrap with .pipe(...)
			replacementNode := genCallNode
			if len(result.PipeArguments) > 0 {
				clonedArgs := make([]*ast.Node, len(result.PipeArguments))
				for i, arg := range result.PipeArguments {
					clonedArgs[i] = tracker.DeepCloneNode(arg)
				}
				pipeAccess := tracker.NewPropertyAccessExpression(genCallNode, nil, tracker.NewIdentifier("pipe"), ast.NodeFlagsNone)
				replacementNode = tracker.NewCallExpression(pipeAccess, nil, nil, tracker.NewNodeList(clonedArgs), ast.NodeFlagsNone)
			}

			// Set parent pointers on the entire replacement tree so the
			// formatter's token-walking code doesn't panic on nil Parent
			ast.SetParentInChildren(replacementNode)

			tracker.ReplaceNode(sf, result.OuterCall.AsNode(), replacementNode, nil)
		},
	}); action != nil {
		return []ls.CodeAction{*action}
	}
	return nil
}
