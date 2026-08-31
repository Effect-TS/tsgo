package change

import (
	"github.com/microsoft/TypeScript/tsc/internal/ast"
	"github.com/microsoft/TypeScript/tsc/internal/core"
	"github.com/microsoft/TypeScript/tsc/internal/ls/change"
	"github.com/microsoft/TypeScript/tsc/internal/lsp/lsproto"
)

func GetChanges(tracker *change.Tracker) map[string][]*lsproto.TextEdit {
	changes, _ := tracker.GetChanges()
	return changes
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
	tracker.ReplaceTextRangeWithText(sourceFile, rng, text)
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
	start := rng.Pos()
	tracker.ReplaceTextRangeWithText(sourceFile, core.NewTextRange(start, start), text)
}
