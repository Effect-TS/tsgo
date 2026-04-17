package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/effect-ts/tsgo/internal/rewriter"
)

var UnnecessaryFailYieldableErrorFix = fixable.Fixable{
	Name:        "unnecessaryFailYieldableError",
	Description: "Replace yield* Effect.fail with yield*",
	ErrorCodes:  []int32{tsdiag.This_yield_Asterisk_Effect_fail_passes_a_yieldable_error_value_yield_Asterisk_represents_that_value_directly_without_wrapping_it_in_Effect_fail_effect_unnecessaryFailYieldableError.Code()},
	FixIDs:      []string{"unnecessaryFailYieldableError_fix"},
	Run:         runUnnecessaryFailYieldableErrorFix,
}

func runUnnecessaryFailYieldableErrorFix(ctx *fixable.Context) []ls.CodeAction {

	c := ctx.Checker

	sf := ctx.SourceFile

	matches := rules.AnalyzeUnnecessaryFailYieldableError(ctx.TypeParser, c, sf)
	for _, match := range matches {
		diagRange := match.Location
		if !diagRange.Intersects(ctx.Span) && !ctx.Span.ContainedBy(diagRange) {
			continue
		}

		// Unwrap "Effect.fail(arg)" to just "arg" by deleting the prefix and suffix around the argument.
		// This keeps "yield*" in place, changing "yield* Effect.fail(error)" to "yield* error".
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace yield* Effect.fail with yield*",
			Run: func(tracker *rewriter.Tracker) {
				tracker.DeleteRange(sf, core.NewTextRange(match.CallNode.Pos(), match.FailArgument.Pos()))
				tracker.DeleteRange(sf, core.NewTextRange(match.FailArgument.End(), match.CallNode.End()))
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
