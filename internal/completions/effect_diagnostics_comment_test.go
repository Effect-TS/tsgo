package completions_test

import (
	"sort"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/rules"
)

func TestEffectDiagnosticsComment_Triggers(t *testing.T) {
	t.Parallel()
	runCommentDirectiveTriggerCases(t, "@effect-diagnostics", "@effect-diagnostics-next-line")
}

func TestEffectDiagnosticsComment_SnippetChoices(t *testing.T) {
	t.Parallel()
	source := "// @"
	items := labeledItems(t, source, len(source), "@effect-diagnostics", "@effect-diagnostics-next-line")
	if len(items) != 2 {
		t.Fatalf("expected 2 items, got %d", len(items))
	}

	names := make([]string, len(rules.All))
	for i, r := range rules.All {
		names[i] = r.Name
	}
	sort.Strings(names)

	insertText := items[0].TextEdit.TextEdit.NewText
	for _, expected := range []string{strings.Join(names, ","), "off,warning,error,message,suggestion"} {
		if !strings.Contains(insertText, expected) {
			t.Errorf("insert text does not contain choices.\ngot:  %q\nwant substring: %q", insertText, expected)
		}
	}
}
