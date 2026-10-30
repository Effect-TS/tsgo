package completions_test

import (
	"sort"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/codegens"
)

func TestEffectCodegensComment_Triggers(t *testing.T) {
	t.Parallel()
	runCommentDirectiveTriggerCases(t, "@effect-codegens")
}

func TestEffectCodegensComment_SnippetContainsSortedCodegenNames(t *testing.T) {
	t.Parallel()
	source := "// @"
	items := labeledItems(t, source, len(source), "@effect-codegens")
	if len(items) != 1 {
		t.Fatalf("expected 1 item, got %d", len(items))
	}

	names := make([]string, len(codegens.All))
	for i, c := range codegens.All {
		names[i] = c.Name
	}
	sort.Strings(names)
	sortedNames := strings.Join(names, ",")

	insertText := items[0].TextEdit.TextEdit.NewText
	if !strings.Contains(insertText, sortedNames) {
		t.Errorf("insert text does not contain sorted codegen names.\ngot:  %q\nwant substring: %q", insertText, sortedNames)
	}
}
