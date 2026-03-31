package completions_test

import (
	"testing"
)

func TestDurationInput_CursorOutsideString(t *testing.T) {
	t.Parallel()
	// Cursor is on a numeric literal, not a string
	source := `const x = 123`
	items := durationInputItems(t, source, len(source))
	if items != nil {
		t.Errorf("expected nil for cursor outside string, got %d items", len(items))
	}
}

func TestDurationInput_CursorOnIdentifier(t *testing.T) {
	t.Parallel()
	// Cursor is on an identifier, not inside a string
	source := `const foo = bar`
	items := durationInputItems(t, source, len(source))
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
	items := durationInputItems(t, source, pos)
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
	items := durationInputItems(t, source, pos)
	if items != nil {
		t.Errorf("expected nil for cursor at closing quote of terminated literal, got %d items", len(items))
	}
}

func TestDurationInput_EmptySource(t *testing.T) {
	t.Parallel()
	source := ``
	items := durationInputItems(t, source, 0)
	if items != nil {
		t.Errorf("expected nil for empty source, got %d items", len(items))
	}
}

func TestDurationInput_CursorInComment(t *testing.T) {
	t.Parallel()
	// Cursor is inside a comment, not a string
	source := `// some comment`
	items := durationInputItems(t, source, len(source))
	if items != nil {
		t.Errorf("expected nil for cursor in comment, got %d items", len(items))
	}
}

func TestDurationInput_CursorBeforeString(t *testing.T) {
	t.Parallel()
	// Cursor is positioned before the string literal (on the equals sign)
	source := `const x = "hello"`
	pos := len(`const x =`)
	items := durationInputItems(t, source, pos)
	if items != nil {
		t.Errorf("expected nil for cursor before string literal, got %d items", len(items))
	}
}

func TestDurationInput_DurationUnitsCount(t *testing.T) {
	t.Parallel()
	// Verify that the durationUnits slice has exactly 8 units
	expected := []string{"nanos", "micros", "millis", "seconds", "minutes", "hours", "days", "weeks"}
	source := "type D = `${number} nanos` | `${number} micros` | `${number} millis` | `${number} seconds` | `${number} minutes` | `${number} hours` | `${number} days` | `${number} weeks`\nconst value: D = \"\""
	items := durationInputItems(t, source, len("type D = `${number} nanos` | `${number} micros` | `${number} millis` | `${number} seconds` | `${number} minutes` | `${number} hours` | `${number} days` | `${number} weeks`\nconst value: D = \""))
	if len(items) != len(expected) {
		t.Fatalf("expected %d duration completions, got %d", len(expected), len(items))
	}
	for _, unit := range expected {
		if findCompletionItem(items, unit) == nil {
			t.Errorf("expected completion label %q", unit)
		}
	}
}
