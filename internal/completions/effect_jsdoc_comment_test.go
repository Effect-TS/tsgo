package completions_test

import (
	"testing"
)

func TestEffectJsdocComment_Triggers(t *testing.T) {
	t.Parallel()
	runCommentDirectiveTriggerCases(t, "@effect-identifier")
}

func TestEffectJsdocComment_InsertText(t *testing.T) {
	t.Parallel()
	source := "// @"
	items := labeledItems(t, source, len(source), "@effect-identifier")
	if len(items) != 1 {
		t.Fatalf("expected 1 item, got %d", len(items))
	}

	insertText := items[0].TextEdit.TextEdit.NewText
	if insertText != "@effect-identifier" {
		t.Errorf("insert text = %q, want %q", insertText, "@effect-identifier")
	}
}
