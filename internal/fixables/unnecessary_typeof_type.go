package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
)

var UnnecessaryTypeofTypeFix = fixable.Fixable{
	Name:        "unnecessaryTypeofType",
	Description: "Replace with the matching named type",
	ErrorCodes:  []int32{tsdiag.This_typeof_Type_query_can_be_replaced_with_0_effect_unnecessaryTypeofType.Code()},
	FixIDs:      []string{"unnecessaryTypeofType_fix"},
	Run:         runUnnecessaryTypeofTypeFix,
}

func runUnnecessaryTypeofTypeFix(ctx *fixable.Context) []ls.CodeAction {
	matches := rules.AnalyzeUnnecessaryTypeofType(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		description := "Replace with '" + match.ReplacementText + "'"
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: description,
			Run: func(tracker *rewriter.Tracker) {
				tracker.ReplaceNode(ctx.SourceFile, match.QueryNode, tracker.DeepCloneNode(match.InnerEntityName), nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
