package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/effect-ts/tsgo/internal/rewriter"
)

var UnnecessaryPipeFix = fixable.Fixable{
	Name:        "unnecessaryPipe",
	Description: "Remove the pipe call",
	ErrorCodes:  []int32{tsdiag.This_pipe_call_contains_no_arguments_effect_unnecessaryPipe.Code()},
	FixIDs:      []string{"unnecessaryPipe_fix"},
	Run:         runUnnecessaryPipeFix,
}

func runUnnecessaryPipeFix(ctx *fixable.Context) []ls.CodeAction {

	c := ctx.Checker

	sf := ctx.SourceFile

	matches := rules.AnalyzeUnnecessaryPipe(ctx.TypeParser, c, sf)

	var match *rules.UnnecessaryPipeMatch
	for i := range matches {
		diagRange := matches[i].Location
		if diagRange.Intersects(ctx.Span) || ctx.Span.ContainedBy(diagRange) {
			match = &matches[i]
			break
		}
	}
	if match == nil {
		return nil
	}

	result := match.Result
	callNode := result.Node.AsNode()

	// Unwrap "pipe(subject)" or "subject.pipe()" to just "subject" by deleting the prefix and suffix around the subject.
	if action := ctx.NewFixAction(fixable.FixAction{
		Description: "Remove the pipe call",
		Run: func(tracker *rewriter.Tracker) {
			tracker.DeleteRange(sf, core.NewTextRange(callNode.Pos(), result.Subject.Pos()))
			tracker.DeleteRange(sf, core.NewTextRange(result.Subject.End(), callNode.End()))
		},
	}); action != nil {
		return []ls.CodeAction{*action}
	}
	return nil
}
