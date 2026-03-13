package completions

import (
	"testing"
)

func TestDurationInput_CursorOutsideString(t *testing.T) {
	t.Parallel()
	// Cursor is on a numeric literal, not a string
	source := `const x = 123`
	ctx := makeFnContext(source, len(source))

	items := runDurationInput(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor outside string, got %d items", len(items))
	}
}

func TestDurationInput_CursorOnIdentifier(t *testing.T) {
	t.Parallel()
	// Cursor is on an identifier, not inside a string
	source := `const foo = bar`
	ctx := makeFnContext(source, len(source))

	items := runDurationInput(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor on identifier, got %d items", len(items))
	}
}

func TestDurationInput_CursorAtOpeningQuote(t *testing.T) {
	t.Parallel()
	// Cursor is at the position of the opening quote (not inside the string content)
	source := `const x: string = "hello"`
	// Position the cursor at the opening quote character
	pos := len(`const x: string = `)
	ctx := makeFnContext(source, pos)

	items := runDurationInput(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor at opening quote, got %d items", len(items))
	}
}

func TestDurationInput_CursorAtClosingQuote(t *testing.T) {
	t.Parallel()
	// Cursor at the closing quote position (end of node) for a terminated literal → not inside
	source := `const x: string = "hello"`
	// Position at the closing quote
	pos := len(`const x: string = "hello"`)
	ctx := makeFnContext(source, pos)

	items := runDurationInput(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor at closing quote of terminated literal, got %d items", len(items))
	}
}

func TestDurationInput_EmptySource(t *testing.T) {
	t.Parallel()
	source := ``
	ctx := makeFnContext(source, 0)

	items := runDurationInput(ctx)
	if items != nil {
		t.Errorf("expected nil for empty source, got %d items", len(items))
	}
}

func TestDurationInput_CursorInComment(t *testing.T) {
	t.Parallel()
	// Cursor is inside a comment, not a string
	source := `// some comment`
	ctx := makeFnContext(source, len(source))

	items := runDurationInput(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor in comment, got %d items", len(items))
	}
}

func TestDurationInput_CursorBeforeString(t *testing.T) {
	t.Parallel()
	// Cursor is positioned before the string literal (on the equals sign)
	source := `const x = "hello"`
	pos := len(`const x =`)
	ctx := makeFnContext(source, pos)

	items := runDurationInput(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor before string literal, got %d items", len(items))
	}
}

func TestDurationInput_DurationUnitsCount(t *testing.T) {
	t.Parallel()
	// Verify that the durationUnits slice has exactly 8 units
	if len(durationUnits) != 8 {
		t.Errorf("expected 8 duration units, got %d", len(durationUnits))
	}

	expected := []string{"nanos", "micros", "millis", "seconds", "minutes", "hours", "days", "weeks"}
	for i, unit := range expected {
		if durationUnits[i] != unit {
			t.Errorf("durationUnits[%d] = %q, want %q", i, durationUnits[i], unit)
		}
	}
}
