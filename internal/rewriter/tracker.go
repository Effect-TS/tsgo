package rewriter

import (
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"strings"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/format"
	"github.com/microsoft/TypeScript/tsc/shim/ls/change"
	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

type NodeOptions = change.NodeOptions
type LeadingTriviaOption = change.LeadingTriviaOption
type TrailingTriviaOption = change.TrailingTriviaOption

const LeadingTriviaOptionNone = change.LeadingTriviaOptionNone
const LeadingTriviaOptionExclude = change.LeadingTriviaOptionExclude
const LeadingTriviaOptionIncludeAll = change.LeadingTriviaOptionIncludeAll
const LeadingTriviaOptionJSDoc = change.LeadingTriviaOptionJSDoc
const LeadingTriviaOptionStartLine = change.LeadingTriviaOptionStartLine

const TrailingTriviaOptionNone = change.TrailingTriviaOptionNone
const TrailingTriviaOptionExclude = change.TrailingTriviaOptionExclude
const TrailingTriviaOptionExcludeWhitespace = change.TrailingTriviaOptionExcludeWhitespace
const TrailingTriviaOptionInclude = change.TrailingTriviaOptionInclude

type pendingBeforeEdit struct {
	sourceFile       *ast.SourceFile
	before           *ast.Node
	text             string
	blankLineBetween bool
	leading          change.LeadingTriviaOption
}

type Tracker struct {
	*change.Tracker
	pendingBefore []pendingBeforeEdit
}

func NewTracker(raw *change.Tracker) *Tracker {
	return &Tracker{Tracker: raw}
}

func (t *Tracker) GetChanges() map[tspath.RootedFilePath][]*lsproto.TextEdit {
	t.flushPendingBefore()
	return change.CollectChanges(t.Tracker)
}

func (t *Tracker) NewModuleDeclaration(
	modifiers *ast.ModifierList,
	keyword ast.Kind,
	name *ast.ModuleName,
	body *ast.ModuleBody,
) *ast.Node {
	return change.NewModuleDeclaration(t.Tracker, modifiers, keyword, name, body)
}

// NewPropertyAccessPath builds the expression a.b.c from ["a", "b", "c"].
func (t *Tracker) NewPropertyAccessPath(names ...string) *ast.Node {
	if len(names) == 0 {
		return nil
	}
	node := t.NewIdentifier(names[0])
	for _, name := range names[1:] {
		node = t.NewPropertyAccessExpression(node, nil, t.NewIdentifier(name), ast.NodeFlagsNone)
	}
	return node
}

// NewQualifiedNamePath builds the type-position name a.b.c from ["a", "b", "c"].
func (t *Tracker) NewQualifiedNamePath(names ...string) *ast.Node {
	if len(names) == 0 {
		return nil
	}
	node := t.NewIdentifier(names[0])
	for _, name := range names[1:] {
		node = t.NewQualifiedName(node, t.NewIdentifier(name))
	}
	return node
}

func (t *Tracker) ReplaceNode(sourceFile *ast.SourceFile, oldNode *ast.Node, newNode *ast.Node, options *change.NodeOptions) {
	if oldNode == nil || newNode == nil {
		return
	}
	if options == nil {
		options = &change.NodeOptions{
			LeadingTriviaOption:  change.LeadingTriviaOptionExclude,
			TrailingTriviaOption: change.TrailingTriviaOptionExclude,
		}
	}
	start := t.GetAdjustedRange(sourceFile, oldNode, oldNode, options.LeadingTriviaOption, options.TrailingTriviaOption).Pos()
	text := t.consumePendingPrefix(sourceFile, oldNode, start) + t.nodeText(sourceFile, newNode)
	if options.Prefix != "" {
		text = options.Prefix + text
	}
	if options.Suffix != "" {
		text += options.Suffix
	}
	change.ReplaceAdjustedRangeWithText(
		t.Tracker,
		sourceFile,
		oldNode,
		oldNode,
		options.LeadingTriviaOption,
		options.TrailingTriviaOption,
		text,
	)
}

// InsertNodeBefore inserts newNode as a statement before the statement
// before. The text is laid out when the edits are collected, so newNode takes
// the indentation of before's line, and a replacement of before keeps it.
func (t *Tracker) InsertNodeBefore(sourceFile *ast.SourceFile, before *ast.Node, newNode *ast.Node, blankLineBetween bool, leadingTriviaOption change.LeadingTriviaOption) {
	if before == nil || newNode == nil {
		return
	}
	t.pendingBefore = append(t.pendingBefore, pendingBeforeEdit{
		sourceFile:       sourceFile,
		before:           before,
		text:             t.nodeText(sourceFile, newNode),
		blankLineBetween: blankLineBetween,
		leading:          leadingTriviaOption,
	})
}

func (t *Tracker) flushPendingBefore() {
	for _, edit := range t.pendingBefore {
		if edit.before == nil || edit.sourceFile == nil || edit.text == "" {
			continue
		}
		start := t.GetAdjustedRange(edit.sourceFile, edit.before, edit.before, edit.leading, change.TrailingTriviaOptionNone).Pos()
		t.ReplaceTextRangeWithText(edit.sourceFile, core.NewTextRange(start, start), pendingBeforeText(edit, start))
	}
	t.pendingBefore = nil
}

// pendingBeforeText lays out the text of edit inserted at pos. When before
// starts its line, the text gets the line's indentation and before moves to the
// next line, indented as it was; when code precedes before on its line, as in
// `{ f() }`, the text is joined with a space.
func pendingBeforeText(edit pendingBeforeEdit, pos int) string {
	sourceText := edit.sourceFile.Text()
	beforeStart := scanner.GetTokenPosOfNode(edit.before, edit.sourceFile, false)
	indentation := sourceText[format.GetLineStartPositionForPosition(beforeStart, edit.sourceFile):beforeStart]
	if strings.TrimLeft(indentation, " \t") != "" {
		return edit.text + " "
	}
	newLines := "\n"
	if edit.blankLineBetween {
		newLines = "\n\n"
	}
	if pos == format.GetLineStartPositionForPosition(pos, edit.sourceFile) {
		return indentation + edit.text + newLines
	}
	return edit.text + newLines + indentation
}

// consumePendingPrefix takes the pending inserts before target, laid out for a
// replacement of target that starts at pos.
func (t *Tracker) consumePendingPrefix(sourceFile *ast.SourceFile, target *ast.Node, pos int) string {
	if target == nil || len(t.pendingBefore) == 0 {
		return ""
	}
	var builder strings.Builder
	kept := t.pendingBefore[:0]
	for _, edit := range t.pendingBefore {
		if edit.sourceFile == sourceFile && edit.before == target {
			builder.WriteString(pendingBeforeText(edit, pos))
			continue
		}
		kept = append(kept, edit)
	}
	t.pendingBefore = kept
	return builder.String()
}

func (t *Tracker) nodeText(sourceFile *ast.SourceFile, node *ast.Node) string {
	text, _ := change.Tracker_getNonformattedText(t.Tracker, node, sourceFile)
	return text
}
