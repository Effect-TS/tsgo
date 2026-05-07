package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
)

var UnsafeEffectTypeAssertionFix = fixable.Fixable{
	Name:        "unsafeEffectTypeAssertion",
	Description: "Remove the unsafe Effect assertion",
	ErrorCodes:  []int32{tsdiag.This_type_assertion_unsafely_narrows_the_Effect_error_or_requirements_channels_effect_unsafeEffectTypeAssertion.Code()},
	FixIDs:      []string{"unsafeEffectTypeAssertion_fix"},
	Run:         runUnsafeEffectTypeAssertionFix,
}

func runUnsafeEffectTypeAssertionFix(ctx *fixable.Context) []ls.CodeAction {
	matches := rules.AnalyzeUnsafeEffectTypeAssertion(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
	for _, match := range matches {
		diagRange := match.Location
		if !diagRange.Intersects(ctx.Span) && !ctx.Span.ContainedBy(diagRange) {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Remove the unsafe Effect assertion",
			Run: func(tracker *rewriter.Tracker) {
				tracker.ReplaceNode(ctx.SourceFile, match.AssertionNode, match.ExpressionNode, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
