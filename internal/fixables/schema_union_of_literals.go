package fixables

import (
	"github.com/effect-ts/effect-typescript-go/internal/fixable"
	"github.com/effect-ts/effect-typescript-go/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var SchemaUnionOfLiteralsFix = fixable.Fixable{
	Name:        "schemaUnionOfLiterals",
	Description: "Replace with a single Schema.Literal call",
	ErrorCodes:  []int32{tsdiag.A_Schema_Union_of_multiple_Schema_Literal_calls_can_be_simplified_to_a_single_Schema_Literal_call_effect_schemaUnionOfLiterals.Code()},
	FixIDs:      []string{"schemaUnionOfLiterals_fix"},
	Run:         runSchemaUnionOfLiteralsFix,
}

func runSchemaUnionOfLiteralsFix(ctx *fixable.Context) []ls.CodeAction {
	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	sf := ctx.SourceFile

	matches := rules.AnalyzeSchemaUnionOfLiterals(c, sf)
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
			Run: func(tracker *change.Tracker) {
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
