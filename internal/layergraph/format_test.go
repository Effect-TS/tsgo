package layergraph

import (
	"testing"
)

func TestMermaidEntityEncode(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{
			name:     "empty string",
			input:    "",
			expected: "",
		},
		{
			name:     "no special chars",
			input:    "hello world",
			expected: "hello world",
		},
		{
			name:     "quotes",
			input:    `"hello"`,
			expected: "#quot;hello#quot;",
		},
		{
			name:     "angle brackets",
			input:    "<div>",
			expected: "#lt;div#gt;",
		},
		{
			name:     "parentheses",
			input:    "fn(x)",
			expected: "fn#40;x#41;",
		},
		{
			name:     "curly braces",
			input:    `{"key":"value"}`,
			expected: "#123;#quot;key#quot;:#quot;value#quot;#125;",
		},
		{
			name:     "hash character",
			input:    "color#red",
			expected: "color#35;red",
		},
		{
			name:     "no double encoding",
			input:    "#",
			expected: "#35;",
		},
		{
			name:     "complex type with special chars",
			input:    `IsGeneric<"With<Special>Chars#!">`,
			expected: "IsGeneric#lt;#quot;With#lt;Special#gt;Chars#35;!#quot;#gt;",
		},
		{
			name:     "JSON edge label",
			input:    `{"relationship":"pipe"}`,
			expected: "#123;#quot;relationship#quot;:#quot;pipe#quot;#125;",
		},
		{
			name:     "JSON edge label with argumentIndex",
			input:    `{"relationship":"call","argumentIndex":0}`,
			expected: "#123;#quot;relationship#quot;:#quot;call#quot;,#quot;argumentIndex#quot;:0#125;",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			result := mermaidEntityEncode(tt.input)
			if result != tt.expected {
				t.Errorf("mermaidEntityEncode(%q) = %q, want %q", tt.input, result, tt.expected)
			}
		})
	}
}

func TestMermaidSafe(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{
			name:     "simple text",
			input:    "hello",
			expected: "hello",
		},
		{
			name:     "newlines replaced with space",
			input:    "hello\nworld",
			expected: "hello world",
		},
		{
			name:     "whitespace collapsed",
			input:    "hello   world",
			expected: "hello world",
		},
		{
			name:     "truncated to 50 chars",
			input:    "a234567890b234567890c234567890d234567890e234567890EXTRA",
			expected: "a234567890b234567890c234567890d234567890e234567890",
		},
		{
			name:     "quotes encoded",
			input:    `say "hello"`,
			expected: "say #quot;hello#quot;",
		},
		{
			name:     "angle brackets encoded",
			input:    "List<number>",
			expected: "List#lt;number#gt;",
		},
		{
			name:     "hash NOT encoded",
			input:    "color#red",
			expected: "color#red",
		},
		{
			name:     "special chars type",
			input:    `IsGeneric<"With<Special>Chars#!">`,
			expected: `IsGeneric#lt;#quot;With#lt;Special#gt;Chars#!#quot;#gt;`,
		},
		{
			name:     "trimmed",
			input:    "  hello  ",
			expected: "hello",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			result := mermaidSafe(tt.input)
			if result != tt.expected {
				t.Errorf("mermaidSafe(%q) = %q, want %q", tt.input, result, tt.expected)
			}
		})
	}
}
