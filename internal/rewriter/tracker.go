package rewriter

import (
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/ls/change"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
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
	sourceFile *ast.SourceFile
	before     *ast.Node
	text       string
	leading    change.LeadingTriviaOption
}

type Tracker struct {
	*change.Tracker
	pendingBefore []pendingBeforeEdit
}

func NewTracker(raw *change.Tracker) *Tracker {
	return &Tracker{Tracker: raw}
}

func (t *Tracker) Raw() *change.Tracker {
	return t.Tracker
}

func (t *Tracker) GetChanges() map[string][]*lsproto.TextEdit {
	t.flushPendingBefore()
	return t.Tracker.GetChanges()
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
	text := t.consumePendingPrefix(sourceFile, oldNode) + t.nodeText(sourceFile, newNode)
	if options.Prefix != "" {
		text = options.Prefix + text
	}
	if options.Suffix != "" {
		text += options.Suffix
	}
	rng := t.GetAdjustedRange(sourceFile, oldNode, oldNode, options.LeadingTriviaOption, options.TrailingTriviaOption)
	t.ReplaceRangeWithText(sourceFile, rng, text)
}

func (t *Tracker) ReplaceNodeWithNodes(sourceFile *ast.SourceFile, oldNode *ast.Node, newNodes []*ast.Node, options *change.NodeOptions) {
	if len(newNodes) == 0 || oldNode == nil {
		return
	}
	if len(newNodes) == 1 {
		newNode := newNodes[0]
		t.ReplaceNode(sourceFile, oldNode, newNode, options)
		return
	}
	if options == nil {
		options = &change.NodeOptions{
			LeadingTriviaOption:  change.LeadingTriviaOptionExclude,
			TrailingTriviaOption: change.TrailingTriviaOptionExclude,
		}
	}
	parts := []string{t.consumePendingPrefix(sourceFile, oldNode)}
	for _, node := range newNodes {
		if node == nil {
			continue
		}
		parts = append(parts, t.nodeText(sourceFile, node))
	}
	text := strings.Join(parts, "\n")
	if options.Prefix != "" {
		text = options.Prefix + text
	}
	if options.Suffix != "" {
		text += options.Suffix
	}
	rng := t.GetAdjustedRange(sourceFile, oldNode, oldNode, options.LeadingTriviaOption, options.TrailingTriviaOption)
	t.ReplaceRangeWithText(sourceFile, rng, text)
}

func (t *Tracker) InsertNodeBefore(sourceFile *ast.SourceFile, before *ast.Node, newNode *ast.Node, blankLineBetween bool, leadingTriviaOption change.LeadingTriviaOption) {
	if before == nil || newNode == nil {
		return
	}
	text := t.nodeText(sourceFile, newNode)
	if blankLineBetween {
		text += "\n\n"
	} else {
		text += "\n"
	}
	t.pendingBefore = append(t.pendingBefore, pendingBeforeEdit{sourceFile: sourceFile, before: before, text: text, leading: leadingTriviaOption})
}

func (t *Tracker) flushPendingBefore() {
	for _, edit := range t.pendingBefore {
		if edit.before == nil || edit.sourceFile == nil || edit.text == "" {
			continue
		}
		start := t.GetAdjustedRange(edit.sourceFile, edit.before, edit.before, edit.leading, change.TrailingTriviaOptionNone).Start
		t.ReplaceRangeWithText(edit.sourceFile, lsproto.Range{Start: start, End: start}, edit.text)
	}
	t.pendingBefore = nil
}

func (t *Tracker) consumePendingPrefix(sourceFile *ast.SourceFile, target *ast.Node) string {
	if target == nil || len(t.pendingBefore) == 0 {
		return ""
	}
	var builder strings.Builder
	kept := t.pendingBefore[:0]
	for _, edit := range t.pendingBefore {
		if edit.sourceFile == sourceFile && edit.before == target {
			builder.WriteString(edit.text)
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
