package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var UnnecessaryArrowBlockFix = fixable.Fixable{
	Name:        "unnecessaryArrowBlock",
	Description: "Use a concise arrow body",
	ErrorCodes:  []int32{tsdiag.This_arrow_function_block_only_returns_an_expression_and_can_use_a_concise_body_effect_unnecessaryArrowBlock.Code()},
	FixIDs:      []string{"unnecessaryArrowBlock_fix"},
	Run:         runUnnecessaryArrowBlockFix,
}

func runUnnecessaryArrowBlockFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile
	matches := rules.AnalyzeUnnecessaryArrowBlock(ctx.TypeParser, ctx.Checker, sf)

	for _, match := range matches {
		diagRange := match.Location
		if !diagRange.Intersects(ctx.Span) && !ctx.Span.ContainedBy(diagRange) {
			continue
		}

		replacementText := "(" + scanner.GetSourceTextOfNodeFromSourceFile(sf, match.ReturnedExpression, false) + ")"
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Use a concise arrow body",
			Run: func(tracker *change.Tracker) {
				tracker.DeleteRange(sf, core.NewTextRange(match.BodyBlock.Pos(), match.BodyBlock.End()))
				tracker.InsertText(sf, ctx.BytePosToLSPPosition(match.BodyBlock.Pos()), replacementText)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
