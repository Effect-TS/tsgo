package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var InstanceOfSchemaFix = fixable.Fixable{
	Name:        "instanceOfSchema",
	Description: "Replace with Schema.is",
	ErrorCodes:  []int32{tsdiag.Consider_using_Schema_is_instead_of_instanceof_for_Effect_Schema_types_effect_instanceOfSchema.Code()},
	FixIDs:      []string{"instanceOfSchema_fix"},
	Run:         runInstanceOfSchemaFix,
}

func runInstanceOfSchemaFix(ctx *fixable.Context) []ls.CodeAction {
	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	sf := ctx.SourceFile

	matches := rules.AnalyzeInstanceOfSchema(c, sf)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Schema.is",
			Run: func(tracker *change.Tracker) {
				clonedLeft := tracker.DeepCloneNode(match.LeftExpr)
				clonedRight := tracker.DeepCloneNode(match.RightExpr)

				// Build Schema.is property access
				schemaIsAccess := tracker.NewPropertyAccessExpression(
					tracker.NewIdentifier("Schema"),
					nil,
					tracker.NewIdentifier("is"),
					ast.NodeFlagsNone,
				)

				// Build inner call: Schema.is(right)
				innerCall := tracker.NewCallExpression(
					schemaIsAccess,
					nil,
					nil,
					tracker.NewNodeList([]*ast.Node{clonedRight}),
					ast.NodeFlagsNone,
				)

				// Build outer call: Schema.is(right)(left)
				outerCall := tracker.NewCallExpression(
					innerCall,
					nil,
					nil,
					tracker.NewNodeList([]*ast.Node{clonedLeft}),
					ast.NodeFlagsNone,
				)

				ast.SetParentInChildren(outerCall)
				tracker.ReplaceNode(sf, match.InstanceOfNode, outerCall, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
