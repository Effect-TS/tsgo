package completions

import (
	"sort"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/codegens"
)

func TestEffectCodegensComment_DoubleSlash(t *testing.T) {
	t.Parallel()
	source := "// @"
	ctx := makeFnContext(source, len(source))

	items := runEffectCodegensComment(ctx)
	if len(items) != 1 {
		t.Fatalf("expected 1 item for '// @', got %d", len(items))
	}
	if items[0].Label != "@effect-codegens" {
		t.Errorf("item[0].Label = %q, want %q", items[0].Label, "@effect-codegens")
	}
}

func TestEffectCodegensComment_SlashStar(t *testing.T) {
	t.Parallel()
	source := "/* @"
	ctx := makeFnContext(source, len(source))

	items := runEffectCodegensComment(ctx)
	if len(items) != 1 {
		t.Fatalf("expected 1 item for '/* @', got %d", len(items))
	}
}

func TestEffectCodegensComment_JSDocComment(t *testing.T) {
	t.Parallel()
	source := "/** @"
	ctx := makeFnContext(source, len(source))

	items := runEffectCodegensComment(ctx)
	if len(items) != 1 {
		t.Fatalf("expected 1 item for '/** @', got %d", len(items))
	}
}

func TestEffectCodegensComment_ExtraWhitespace(t *testing.T) {
	t.Parallel()
	source := "//  @  "
	ctx := makeFnContext(source, len(source))

	items := runEffectCodegensComment(ctx)
	if len(items) != 1 {
		t.Fatalf("expected 1 item for '//  @  ' (extra whitespace), got %d", len(items))
	}
}

func TestEffectCodegensComment_NoAtSymbol(t *testing.T) {
	t.Parallel()
	source := "// some comment without at"
	ctx := makeFnContext(source, len(source))

	items := runEffectCodegensComment(ctx)
	if items != nil {
		t.Errorf("expected nil for comment without @, got %d items", len(items))
	}
}

func TestEffectCodegensComment_AtOutsideComment(t *testing.T) {
	t.Parallel()
	source := "const x = @"
	ctx := makeFnContext(source, len(source))

	items := runEffectCodegensComment(ctx)
	if items != nil {
		t.Errorf("expected nil for @ outside comment, got %d items", len(items))
	}
}

func TestEffectCodegensComment_SnippetContainsSortedCodegenNames(t *testing.T) {
	t.Parallel()
	source := "// @"
	ctx := makeFnContext(source, len(source))

	items := runEffectCodegensComment(ctx)
	if len(items) != 1 {
		t.Fatalf("expected 1 item, got %d", len(items))
	}

	// Build expected sorted names
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

func TestEffectCodegensComment_ReplacementSpanStartsAtAt(t *testing.T) {
	t.Parallel()
	source := "// @"
	ctx := makeFnContext(source, len(source))

	items := runEffectCodegensComment(ctx)
	if len(items) != 1 {
		t.Fatalf("expected 1 item, got %d", len(items))
	}

	// The @ is at byte offset 3 in "// @", which is line 0, character 3
	rang := items[0].TextEdit.TextEdit.Range
	if rang.Start.Character != 3 {
		t.Errorf("replacement range start character = %d, want 3", rang.Start.Character)
	}
}

func TestEffectCodegensComment_MultilineWithCommentOnSecondLine(t *testing.T) {
	t.Parallel()
	source := "const x = 1\n// @"
	ctx := makeFnContext(source, len(source))

	items := runEffectCodegensComment(ctx)
	if len(items) != 1 {
		t.Fatalf("expected 1 item for multiline source, got %d", len(items))
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
