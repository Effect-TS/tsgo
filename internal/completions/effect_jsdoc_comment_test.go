package completions

import (
	"testing"
)

func TestEffectJsdocComment_DoubleSlash(t *testing.T) {
	t.Parallel()
	source := "// @"
	ctx := makeFnContext(source, len(source))

	items := runEffectJsdocComment(ctx)
	if len(items) != 1 {
		t.Fatalf("expected 1 item for '// @', got %d", len(items))
	}
	if items[0].Label != "@effect-identifier" {
		t.Errorf("item[0].Label = %q, want %q", items[0].Label, "@effect-identifier")
	}
}

func TestEffectJsdocComment_SlashStar(t *testing.T) {
	t.Parallel()
	source := "/* @"
	ctx := makeFnContext(source, len(source))

	items := runEffectJsdocComment(ctx)
	if len(items) != 1 {
		t.Fatalf("expected 1 item for '/* @', got %d", len(items))
	}
	if items[0].Label != "@effect-identifier" {
		t.Errorf("item[0].Label = %q, want %q", items[0].Label, "@effect-identifier")
	}
}

func TestEffectJsdocComment_JSDocComment(t *testing.T) {
	t.Parallel()
	source := "/** @"
	ctx := makeFnContext(source, len(source))

	items := runEffectJsdocComment(ctx)
	if len(items) != 1 {
		t.Fatalf("expected 1 item for '/** @', got %d", len(items))
	}
}

func TestEffectJsdocComment_ExtraWhitespace(t *testing.T) {
	t.Parallel()
	source := "//  @  "
	ctx := makeFnContext(source, len(source))

	items := runEffectJsdocComment(ctx)
	if len(items) != 1 {
		t.Fatalf("expected 1 item for '//  @  ' (extra whitespace), got %d", len(items))
	}
}

func TestEffectJsdocComment_NoAtSymbol(t *testing.T) {
	t.Parallel()
	source := "// some comment"
	ctx := makeFnContext(source, len(source))

	items := runEffectJsdocComment(ctx)
	if items != nil {
		t.Errorf("expected nil for comment without @, got %d items", len(items))
	}
}

func TestEffectJsdocComment_AtOutsideComment(t *testing.T) {
	t.Parallel()
	source := "const x = @"
	ctx := makeFnContext(source, len(source))

	items := runEffectJsdocComment(ctx)
	if items != nil {
		t.Errorf("expected nil for @ outside comment, got %d items", len(items))
	}
}

func TestEffectJsdocComment_InsertText(t *testing.T) {
	t.Parallel()
	source := "// @"
	ctx := makeFnContext(source, len(source))

	items := runEffectJsdocComment(ctx)
	if len(items) != 1 {
		t.Fatalf("expected 1 item, got %d", len(items))
	}

	insertText := items[0].TextEdit.TextEdit.NewText
	if insertText != "@effect-identifier" {
		t.Errorf("insert text = %q, want %q", insertText, "@effect-identifier")
	}
}

func TestEffectJsdocComment_ReplacementSpanStartsAtAt(t *testing.T) {
	t.Parallel()
	source := "// @"
	ctx := makeFnContext(source, len(source))

	items := runEffectJsdocComment(ctx)
	if len(items) != 1 {
		t.Fatalf("expected 1 item, got %d", len(items))
	}

	// The @ is at byte offset 3 in "// @", which is line 0, character 3
	rang := items[0].TextEdit.TextEdit.Range
	if rang.Start.Character != 3 {
		t.Errorf("replacement range start character = %d, want 3", rang.Start.Character)
	}
}

func TestEffectJsdocComment_MultilineWithCommentOnSecondLine(t *testing.T) {
	t.Parallel()
	source := "const x = 1\n// @"
	ctx := makeFnContext(source, len(source))

	items := runEffectJsdocComment(ctx)
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
