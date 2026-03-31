package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var UnnecessaryFailYieldableErrorFix = fixable.Fixable{
	Name:        "unnecessaryFailYieldableError",
	Description: "Replace yield* Effect.fail with yield*",
	ErrorCodes:  []int32{tsdiag.This_Effect_fail_call_uses_a_yieldable_error_type_as_argument_You_can_yield_Asterisk_the_error_directly_instead_effect_unnecessaryFailYieldableError.Code()},
	FixIDs:      []string{"unnecessaryFailYieldableError_fix"},
	Run:         runUnnecessaryFailYieldableErrorFix,
}

func runUnnecessaryFailYieldableErrorFix(ctx *fixable.Context) []ls.CodeAction {

	c := ctx.Checker

	sf := ctx.SourceFile

	matches := rules.AnalyzeUnnecessaryFailYieldableError(c, sf)
	for _, match := range matches {
		diagRange := match.Location
		if !diagRange.Intersects(ctx.Span) && !ctx.Span.ContainedBy(diagRange) {
			continue
		}

		// Unwrap "Effect.fail(arg)" to just "arg" by deleting the prefix and suffix around the argument.
		// This keeps "yield*" in place, changing "yield* Effect.fail(error)" to "yield* error".
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace yield* Effect.fail with yield*",
			Run: func(tracker *change.Tracker) {
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
