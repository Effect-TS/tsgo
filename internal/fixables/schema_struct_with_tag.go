package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var SchemaStructWithTagFix = fixable.Fixable{
	Name:        "schemaStructWithTag",
	Description: "Rewrite as Schema.TaggedStruct",
	ErrorCodes:  []int32{tsdiag.This_Schema_Struct_includes_a_tag_field_Schema_TaggedStruct_is_the_tagged_struct_form_for_this_pattern_and_makes_the_tag_optional_in_the_constructor_effect_schemaStructWithTag.Code()},
	FixIDs:      []string{"schemaStructWithTag_fix"},
	Run:         runSchemaStructWithTagFix,
}

func runSchemaStructWithTagFix(ctx *fixable.Context) []ls.CodeAction {
	c := ctx.Checker

	sf := ctx.SourceFile

	matches := rules.AnalyzeSchemaStructWithTag(ctx.TypeParser, c, sf)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		// Resolve the Schema module name, preserving import aliases
		schemaModuleName := "Schema"
		if match.SchemaExpr != nil && match.SchemaExpr.Kind == ast.KindIdentifier {
			schemaModuleName = scanner.GetTextOfNode(match.SchemaExpr)
		}

		// Capture loop variables for the closure
		tagValue := match.TagValue
		otherProperties := match.OtherProperties
		callNode := match.CallNode

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Rewrite as Schema.TaggedStruct",
			Run: func(tracker *change.Tracker) {
				// Build Schema.TaggedStruct property access
				schemaId := tracker.NewIdentifier(schemaModuleName)
				taggedStructAccess := tracker.NewPropertyAccessExpression(
					schemaId, nil, tracker.NewIdentifier("TaggedStruct"), ast.NodeFlagsNone,
				)

				// Build the tag value string literal argument
				tagLiteral := tracker.NewStringLiteral(tagValue, ast.TokenFlagsNone)

				// Deep-clone each remaining property and build the new object literal
				clonedProps := make([]*ast.Node, len(otherProperties))
				for i, prop := range otherProperties {
					clonedProps[i] = tracker.DeepCloneNode(prop)
				}
				newObjLiteral := tracker.NewObjectLiteralExpression(
					tracker.NewNodeList(clonedProps),
					false,
				)

				// Build Schema.TaggedStruct("tagValue", { ...rest })
				newCall := tracker.NewCallExpression(
					taggedStructAccess, nil, nil,
					tracker.NewNodeList([]*ast.Node{tagLiteral, newObjLiteral}),
					ast.NodeFlagsNone,
				)

				ast.SetParentInChildren(newCall)
				tracker.ReplaceNode(sf, callNode, newCall, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
