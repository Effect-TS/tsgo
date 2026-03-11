// Package fixables contains all code fix implementations.
package fixables

import (
	"github.com/effect-ts/effect-typescript-go/internal/fixable"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

// EffectDisable provides "disable for line/file" actions for all Effect diagnostics.
// This fixable handles all Effect diagnostic codes and provides two actions:
// 1. "Disable {ruleName} for this line" - inserts @effect-diagnostics-next-line directive
// 2. "Disable {ruleName} for entire file" - inserts @effect-diagnostics skip-file directive
var EffectDisable = fixable.Fixable{
	Name:        "effectDisable",
	Description: "Disable Effect diagnostics via directive comments",
	ErrorCodes:  rule.AllCodes(rules.All),
	FixIDs:      []string{"effectDisableNextLine", "effectDisableFile"},
	Run:         runEffectDisable,
}

// runEffectDisable generates disable actions for Effect diagnostics.
func runEffectDisable(ctx *fixable.Context) []ls.CodeAction {
	ruleName := rule.CodeToRuleName(rules.All, ctx.ErrorCode)
	if ruleName == "" {
		return nil
	}

	var actions []ls.CodeAction

	// "Disable for this line" action
	if action := createDisableNextLineAction(ctx, ruleName); action != nil {
		actions = append(actions, *action)
	}

	// "Disable for entire file" action
	if action := createDisableFileAction(ctx, ruleName); action != nil {
		actions = append(actions, *action)
	}

	return actions
}

// createDisableNextLineAction creates a code action to disable the diagnostic for the next line.
// It inserts: // @effect-diagnostics-next-line {ruleName}:off
func createDisableNextLineAction(ctx *fixable.Context, ruleName string) *ls.CodeAction {
	sourceFile := ctx.SourceFile
	span := ctx.Span

	// Find the start of the line containing the diagnostic
	lineStartPos := getLineStartPositionForPosition(span.Pos(), sourceFile)

	// Get the indentation at this position
	indent := getIndentationAtPosition(sourceFile, lineStartPos)

	// Create the comment text with proper indentation
	comment := indent + "// @effect-diagnostics-next-line " + ruleName + ":off\n"

	// Insert at the start of the line
	insertPos := ctx.BytePosToLSPPosition(lineStartPos)
	// Character must be 0 since we're inserting at line start with full indentation in the text
	insertPos.Character = 0

	return ctx.NewFixAction(fixable.FixAction{
		Description: "Disable " + ruleName + " for this line",
		Run: func(tracker *change.Tracker) {
			tracker.InsertText(sourceFile, insertPos, comment)
		},
	})
}

// createDisableFileAction creates a code action to disable the diagnostic for the entire file.
// It inserts: /** @effect-diagnostics {ruleName}:off */ at the top of the file.
func createDisableFileAction(ctx *fixable.Context, ruleName string) *ls.CodeAction {
	// Create the comment text (uses :off for consistency with next-line directives)
	comment := "/** @effect-diagnostics " + ruleName + ":off */\n"

	// Insert at the very beginning of the file (position 0)
	insertPos := ctx.BytePosToLSPPosition(0)

	return ctx.NewFixAction(fixable.FixAction{
		Description: "Disable " + ruleName + " for entire file",
		Run: func(tracker *change.Tracker) {
			tracker.InsertText(ctx.SourceFile, insertPos, comment)
		},
	})
}

// getLineStartPositionForPosition finds the start of the line containing the given position.
func getLineStartPositionForPosition(pos int, sourceFile *ast.SourceFile) int {
	text := sourceFile.Text()
	if pos <= 0 || pos > len(text) {
		return 0
	}
	// Walk backwards to find the newline
	for i := pos - 1; i >= 0; i-- {
		if text[i] == '\n' {
			return i + 1
		}
	}
	return 0
}

// getIndentationAtPosition returns the whitespace characters from line start to the first non-whitespace.
func getIndentationAtPosition(sourceFile *ast.SourceFile, lineStartPos int) string {
	text := sourceFile.Text()
	if lineStartPos >= len(text) {
		return ""
	}

	// Find the end of indentation (first non-whitespace character or end of line)
	endPos := lineStartPos
	for endPos < len(text) {
		ch := text[endPos]
		if ch == ' ' || ch == '\t' {
			endPos++
		} else {
			break
		}
	}

	return text[lineStartPos:endPos]
}
