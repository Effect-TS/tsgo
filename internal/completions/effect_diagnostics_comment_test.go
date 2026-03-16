package completions

import (
	"sort"
	"strings"
	"testing"

	"github.com/effect-ts/effect-typescript-go/internal/rules"
)

func TestEffectDiagnosticsComment_DoubleSlash(t *testing.T) {
	t.Parallel()
	source := "// @"
	ctx := makeFnContext(source, len(source))

	items := runEffectDiagnosticsComment(ctx)
	if len(items) != 2 {
		t.Fatalf("expected 2 items for '// @', got %d", len(items))
	}
	if items[0].Label != "@effect-diagnostics" {
		t.Errorf("item[0].Label = %q, want %q", items[0].Label, "@effect-diagnostics")
	}
	if items[1].Label != "@effect-diagnostics-next-line" {
		t.Errorf("item[1].Label = %q, want %q", items[1].Label, "@effect-diagnostics-next-line")
	}
}

func TestEffectDiagnosticsComment_SlashStar(t *testing.T) {
	t.Parallel()
	source := "/* @"
	ctx := makeFnContext(source, len(source))

	items := runEffectDiagnosticsComment(ctx)
	if len(items) != 2 {
		t.Fatalf("expected 2 items for '/* @', got %d", len(items))
	}
	if items[0].Label != "@effect-diagnostics" {
		t.Errorf("item[0].Label = %q, want %q", items[0].Label, "@effect-diagnostics")
	}
	if items[1].Label != "@effect-diagnostics-next-line" {
		t.Errorf("item[1].Label = %q, want %q", items[1].Label, "@effect-diagnostics-next-line")
	}
}

func TestEffectDiagnosticsComment_JSDocComment(t *testing.T) {
	t.Parallel()
	source := "/** @"
	ctx := makeFnContext(source, len(source))

	items := runEffectDiagnosticsComment(ctx)
	if len(items) != 2 {
		t.Fatalf("expected 2 items for '/** @', got %d", len(items))
	}
}

func TestEffectDiagnosticsComment_ExtraWhitespace(t *testing.T) {
	t.Parallel()
	source := "//  @  "
	ctx := makeFnContext(source, len(source))

	items := runEffectDiagnosticsComment(ctx)
	if len(items) != 2 {
		t.Fatalf("expected 2 items for '//  @  ' (extra whitespace), got %d", len(items))
	}
}

func TestEffectDiagnosticsComment_NoAtSymbol(t *testing.T) {
	t.Parallel()
	source := "// some comment without at"
	ctx := makeFnContext(source, len(source))

	items := runEffectDiagnosticsComment(ctx)
	if items != nil {
		t.Errorf("expected nil for comment without @, got %d items", len(items))
	}
}

func TestEffectDiagnosticsComment_AtOutsideComment(t *testing.T) {
	t.Parallel()
	source := "const x = @"
	ctx := makeFnContext(source, len(source))

	items := runEffectDiagnosticsComment(ctx)
	if items != nil {
		t.Errorf("expected nil for @ outside comment, got %d items", len(items))
	}
}

func TestEffectDiagnosticsComment_SnippetContainsSortedRuleNames(t *testing.T) {
	t.Parallel()
	source := "// @"
	ctx := makeFnContext(source, len(source))

	items := runEffectDiagnosticsComment(ctx)
	if len(items) != 2 {
		t.Fatalf("expected 2 items, got %d", len(items))
	}

	// Build expected sorted names
	names := make([]string, len(rules.All))
	for i, r := range rules.All {
		names[i] = r.Name
	}
	sort.Strings(names)
	sortedNames := strings.Join(names, ",")

	insertText := items[0].TextEdit.TextEdit.NewText
	if !strings.Contains(insertText, sortedNames) {
		t.Errorf("insert text does not contain sorted rule names.\ngot:  %q\nwant substring: %q", insertText, sortedNames)
	}
}

func TestEffectDiagnosticsComment_SnippetContainsSeverityChoices(t *testing.T) {
	t.Parallel()
	source := "// @"
	ctx := makeFnContext(source, len(source))

	items := runEffectDiagnosticsComment(ctx)
	if len(items) != 2 {
		t.Fatalf("expected 2 items, got %d", len(items))
	}

	expected := "off,warning,error,message,suggestion"
	insertText := items[0].TextEdit.TextEdit.NewText
	if !strings.Contains(insertText, expected) {
		t.Errorf("insert text does not contain severity choices.\ngot:  %q\nwant substring: %q", insertText, expected)
	}
}

func TestEffectDiagnosticsComment_ReplacementSpanStartsAtAt(t *testing.T) {
	t.Parallel()
	source := "// @"
	ctx := makeFnContext(source, len(source))

	items := runEffectDiagnosticsComment(ctx)
	if len(items) != 2 {
		t.Fatalf("expected 2 items, got %d", len(items))
	}

	// The @ is at byte offset 3 in "// @", which is line 0, character 3
	rang := items[0].TextEdit.TextEdit.Range
	if rang.Start.Character != 3 {
		t.Errorf("replacement range start character = %d, want 3", rang.Start.Character)
	}
}

func TestEffectDiagnosticsComment_MultilineWithCommentOnSecondLine(t *testing.T) {
	t.Parallel()
	source := "const x = 1\n// @"
	ctx := makeFnContext(source, len(source))

	items := runEffectDiagnosticsComment(ctx)
	if len(items) != 2 {
		t.Fatalf("expected 2 items for multiline source, got %d", len(items))
	}

	// @ is at character 3 on line 1
	rang := items[0].TextEdit.TextEdit.Range
	if rang.Start.Line != 1 {
		t.Errorf("replacement range start line = %d, want 1", rang.Start.Line)
	}
	if rang.Start.Character != 3 {
		t.Errorf("replacement range start character = %d, want 3", rang.Start.Character)
	}
}
