package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var DiscardedRunPromiseToRunForkFix = fixable.Fixable{
	Name:        "discardedRunPromiseToRunFork",
	Description: "Replace with the fire-and-forget Fiber runner",
	ErrorCodes: []int32{
		tsdiag.The_Promise_returned_by_0_is_discarded_so_any_failure_or_defect_becomes_an_unhandled_rejection_Use_1_to_run_it_fire_and_forget_as_a_Fiber_effect_discardedRunPromiseToRunFork.Code(),
	},
	FixIDs: []string{"discardedRunPromiseToRunFork_fix"},
	Run:    runDiscardedRunPromiseToRunForkFix,
}

func runDiscardedRunPromiseToRunForkFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeDiscardedRunPromiseToRunFork(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if match.RenameNode == nil {
			continue
		}

		// The result is discarded, so renaming the runner in place keeps the
		// receiver, the arguments and any surrounding void untouched.
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with " + match.ReplacementName,
			Run: func(tracker *rewriter.Tracker) {
				tracker.ReplaceNode(ctx.SourceFile, match.RenameNode, tracker.NewIdentifier(match.RenameTo), nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}
