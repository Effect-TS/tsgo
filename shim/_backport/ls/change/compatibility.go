package change

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/ls/change"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

func GetChanges(tracker *change.Tracker) map[string][]*lsproto.TextEdit {
	return tracker.GetChanges()
}

func ReplaceAdjustedRangeWithText(
	tracker *change.Tracker,
	sourceFile *ast.SourceFile,
	startNode *ast.Node,
	endNode *ast.Node,
	leadingOption change.LeadingTriviaOption,
	trailingOption change.TrailingTriviaOption,
	text string,
) {
	rng := tracker.GetAdjustedRange(sourceFile, startNode, endNode, leadingOption, trailingOption)
	tracker.ReplaceRangeWithText(sourceFile, rng, text)
}

func InsertTextAtAdjustedRangeStart(
	tracker *change.Tracker,
	sourceFile *ast.SourceFile,
	startNode *ast.Node,
	endNode *ast.Node,
	leadingOption change.LeadingTriviaOption,
	trailingOption change.TrailingTriviaOption,
	text string,
) {
	rng := tracker.GetAdjustedRange(sourceFile, startNode, endNode, leadingOption, trailingOption)
	tracker.ReplaceRangeWithText(sourceFile, lsproto.Range{Start: rng.Start, End: rng.Start}, text)
}
