package change

import (
	"github.com/microsoft/typescript-go/internal/ast"
	"github.com/microsoft/typescript-go/internal/core"
	"github.com/microsoft/typescript-go/internal/ls/change"
	"github.com/microsoft/typescript-go/internal/lsp/lsproto"
)

func GetChanges(tracker *change.Tracker) map[string][]*lsproto.TextEdit {
	changes, _ := tracker.GetChanges()
	return changes
}

func NewModuleDeclaration(
	tracker *change.Tracker,
	modifiers *ast.ModifierList,
	keyword ast.Kind,
	name *ast.ModuleName,
	body *ast.ModuleBody,
) *ast.Node {
	return tracker.NewModuleDeclaration(modifiers, keyword, name, nil, body)
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
