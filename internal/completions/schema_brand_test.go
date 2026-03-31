package completions_test

import (
	"testing"
)

func TestSchemaBrand_EmptySource(t *testing.T) {
	t.Parallel()
	source := ``
	items := schemaBrandItems(t, source, 0)
	if items != nil {
		t.Errorf("expected nil for empty source, got %d items", len(items))
	}
}

func TestSchemaBrand_CursorAtStartOfFile(t *testing.T) {
	t.Parallel()
	source := `const x = 1`
	items := schemaBrandItems(t, source, 0)
	if items != nil {
		t.Errorf("expected nil for cursor at start of file, got %d items", len(items))
	}
}

func TestSchemaBrand_InsideImportDeclaration(t *testing.T) {
	t.Parallel()
	// Cursor inside an import declaration should not trigger completion
	source := `import { Schema } from "effect"`
	pos := len(`import { Schema`)
	items := schemaBrandItems(t, source, pos)
	if items != nil {
		t.Errorf("expected nil for cursor inside import declaration, got %d items", len(items))
	}
}

func TestSchemaBrand_CursorAfterNumber(t *testing.T) {
	t.Parallel()
	// Cursor after a non-identifier token — ParseAccessedExpressionForCompletion returns nil
	source := `const x = 42`
	items := schemaBrandItems(t, source, len(source))
	if items != nil {
		t.Errorf("expected nil for cursor after number literal, got %d items", len(items))
	}
}

func TestSchemaBrand_CursorAfterString(t *testing.T) {
	t.Parallel()
	// Cursor after a string literal — not a dot-access or identifier
	source := `const x = "hello"`
	items := schemaBrandItems(t, source, len(source))
	if items != nil {
		t.Errorf("expected nil for cursor after string literal, got %d items", len(items))
	}
}
