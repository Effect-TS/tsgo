package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/effect-ts/tsgo/internal/rewriter"
)

var SchemaUnionOfLiteralsFix = fixable.Fixable{
	Name:        "schemaUnionOfLiterals",
	Description: "Replace with a single Schema.Literal call",
	ErrorCodes:  []int32{tsdiag.This_Schema_Union_contains_multiple_Schema_Literal_members_and_can_be_simplified_to_a_single_Schema_Literal_call_effect_schemaUnionOfLiterals.Code()},
	FixIDs:      []string{"schemaUnionOfLiterals_fix"},
	Run:         runSchemaUnionOfLiteralsFix,
}

func runSchemaUnionOfLiteralsFix(ctx *fixable.Context) []ls.CodeAction {
	c := ctx.Checker

	sf := ctx.SourceFile

	matches := rules.AnalyzeSchemaUnionOfLiterals(ctx.TypeParser, c, sf)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		// Capture loop variables for the closure
		unionCallNode := match.UnionCallNode
		firstLiteralExpression := match.FirstLiteralExpression
		allLiteralArgs := match.AllLiteralArgs

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with a single Schema.Literal call",
			Run: func(tracker *rewriter.Tracker) {
				clonedExpression := tracker.DeepCloneNode(firstLiteralExpression)
				clonedArgs := make([]*ast.Node, len(allLiteralArgs))
				for i, arg := range allLiteralArgs {
					clonedArgs[i] = tracker.DeepCloneNode(arg)
				}
				newCall := tracker.NewCallExpression(
					clonedExpression, nil, nil,
					tracker.NewNodeList(clonedArgs),
					ast.NodeFlagsNone,
				)
				ast.SetParentInChildren(newCall)
				tracker.ReplaceNode(sf, unionCallNode, newCall, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
