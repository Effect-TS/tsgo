package completions_test

import (
	"testing"
)

var durationUnits = []string{"nanos", "micros", "millis", "seconds", "minutes", "hours", "days", "weeks"}

func TestDurationInput_NotInsideStringContent(t *testing.T) {
	t.Parallel()
	for _, tc := range []struct {
		name   string
		source string
		cursor string
	}{
		{"numeric literal", `const x = 123`, ""},
		{"identifier", `const foo = bar`, ""},
		{"opening quote", `const x: string = "hello"`, `const x: string = `},
		{"closing quote of terminated literal", `const x: string = "hello"`, ""},
		{"empty source", ``, ""},
		{"comment", `// some comment`, ""},
		{"before string literal", `const x = "hello"`, `const x =`},
	} {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			position := len(tc.source)
			if tc.cursor != "" {
				position = len(tc.cursor)
			}
			if items := labeledItems(t, tc.source, position, durationUnits...); items != nil {
				t.Errorf("expected no duration completions, got %d items", len(items))
			}
		})
	}
}

func TestDurationInput_DurationUnitsCount(t *testing.T) {
	t.Parallel()
	prefix := "type D = `${number} nanos` | `${number} micros` | `${number} millis` | `${number} seconds` | `${number} minutes` | `${number} hours` | `${number} days` | `${number} weeks`\nconst value: D = \""
	source := prefix + "\""
	items := labeledItems(t, source, len(prefix), durationUnits...)
	if len(items) != len(durationUnits) {
		t.Fatalf("expected %d duration completions, got %d", len(durationUnits), len(items))
	}
	for _, unit := range durationUnits {
		if findCompletionItem(items, unit) == nil {
			t.Errorf("expected completion label %q", unit)
		}
	}
}
