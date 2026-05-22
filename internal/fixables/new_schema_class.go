package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
)

var NewSchemaClassFix = fixable.Fixable{
	Name:        "newSchemaClass",
	Description: "Replace new Schema class construction with .make",
	ErrorCodes:  []int32{tsdiag.This_Schema_class_is_constructed_with_new_0_make_can_be_used_to_construct_the_Schema_class_instance_effect_newSchemaClass.Code()},
	FixIDs:      []string{"newSchemaClass_fix"},
	Run:         runNewSchemaClassFix,
}

func runNewSchemaClassFix(ctx *fixable.Context) []ls.CodeAction {
	matches := rules.AnalyzeNewSchemaClass(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with .make",
			Run: func(tracker *rewriter.Tracker) {
				clonedClassExpr := tracker.DeepCloneNode(match.ClassExpr)
				makeAccess := tracker.NewPropertyAccessExpression(
					clonedClassExpr,
					nil,
					tracker.NewIdentifier("make"),
					ast.NodeFlagsNone,
				)

				var clonedArgs []*ast.Node
				if len(match.Arguments) > 0 {
					clonedArgs = make([]*ast.Node, len(match.Arguments))
					for i, arg := range match.Arguments {
						clonedArgs[i] = tracker.DeepCloneNode(arg)
					}
				}

				replacement := tracker.NewCallExpression(
					makeAccess,
					nil,
					nil,
					tracker.NewNodeList(clonedArgs),
					ast.NodeFlagsNone,
				)

				ast.SetParentInChildren(replacement)
				tracker.ReplaceNode(ctx.SourceFile, match.NewNode, replacement, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
