package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var ZipViaNestedFlatMapToAllFix = fixable.Fixable{
	Name:        "zipViaNestedFlatMapToAll",
	Description: "Replace with Effect.all",
	ErrorCodes: []int32{
		tsdiag.This_nested_Effect_flatMap_Slash_Effect_map_only_pairs_two_independent_effects_use_Effect_all_sequential_by_default_to_express_this_directly_effect_zipViaNestedFlatMapToAll.Code(),
	},
	FixIDs: []string{"zipViaNestedFlatMapToAll_fix"},
	Run:    runZipViaNestedFlatMapToAllFix,
}

func runZipViaNestedFlatMapToAllFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeZipViaNestedFlatMapToAll(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if !match.Fixable {
			continue
		}
		all := effectModuleMember(ctx, "all")
		allReference := resolveModuleMember(ctx, match.Callee, typeparser.FindEffectModuleReference(ctx.SourceFile), all)
		if allReference == nil {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.all",
			Run: func(tracker *rewriter.Tracker) {
				a := zipOperandExpression(tracker, match.A)
				b := zipOperandExpression(tracker, match.B)
				var argument *ast.Node
				if match.Record {
					argument = tracker.NewObjectLiteralExpression(tracker.NewNodeList([]*ast.Node{
						tracker.NewPropertyAssignment(nil, tracker.DeepCloneNode(match.Keys[0]), nil, nil, a),
						tracker.NewPropertyAssignment(nil, tracker.DeepCloneNode(match.Keys[1]), nil, nil, b),
					}), false)
				} else {
					argument = tracker.NewArrayLiteralExpression(tracker.NewNodeList([]*ast.Node{a, b}), false)
				}
				call := tracker.NewCallExpression(
					allReference.expression(tracker, all),
					nil,
					nil,
					tracker.NewNodeList([]*ast.Node{argument}),
					ast.NodeFlagsNone,
				)
				// Effect.all replaces the outer flow up to the flatMap step;
				// later steps then apply to its result.
				tracker.ReplacePipingFlowPrefix(ctx.SourceFile, match.Flow, match.Index+1, call)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}

// zipOperandExpression rebuilds an Effect.all operand: a clone of the
// standalone expression, or the pipe call truncated before the step.
func zipOperandExpression(tracker *rewriter.Tracker, operand rules.ZipViaNestedFlatMapToAllOperand) *ast.Node {
	if operand.Node != nil {
		return tracker.DeepCloneNode(operand.Node)
	}
	parts := operand.Parts()
	arguments := make([]*ast.Node, 0, len(parts)-1)
	for _, argument := range parts[1:] {
		arguments = append(arguments, tracker.DeepCloneNode(argument))
	}
	return tracker.NewCallExpression(
		tracker.DeepCloneNode(parts[0]),
		nil,
		nil,
		tracker.NewNodeList(arguments),
		ast.NodeFlagsNone,
	)
}
