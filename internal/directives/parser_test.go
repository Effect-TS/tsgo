package directives

import (
	"testing"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

func TestEtscoreParseSeverity(t *testing.T) {
	t.Parallel()
	const fallbackSeverity = etscore.SeverityError
	tests := []struct {
		input    string
		expected etscore.Severity
	}{
		{"off", etscore.SeverityOff},
		{"OFF", etscore.SeverityOff},
		{"warning", etscore.SeverityWarning},
		{"warn", etscore.SeverityWarning},
		{"WARN", etscore.SeverityWarning},
		{"error", etscore.SeverityError},
		{"ERROR", etscore.SeverityError},
		{"suggestion", etscore.SeveritySuggestion},
		{"SUGGESTION", etscore.SeveritySuggestion},
		{"Suggestion", etscore.SeveritySuggestion},
		{"message", etscore.SeverityMessage},
		{"MESSAGE", etscore.SeverityMessage},
		{"Message", etscore.SeverityMessage},
		{"skip-file", etscore.SeveritySkipFile},
		{"SKIP-FILE", etscore.SeveritySkipFile},
		{"unknown", fallbackSeverity},
		{"", fallbackSeverity},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			t.Parallel()
			result := etscore.ParseSeverity(tt.input)
			if result != tt.expected {
				t.Errorf("etscore.ParseSeverity(%q) = %v, want %v", tt.input, result, tt.expected)
			}
		})
	}
}

func TestEtscoreSeverityString(t *testing.T) {
	t.Parallel()
	tests := []struct {
		severity etscore.Severity
		expected string
	}{
		{etscore.SeverityOff, "off"},
		{etscore.SeverityWarning, "warning"},
		{etscore.SeverityError, "error"},
		{etscore.SeveritySuggestion, "suggestion"},
		{etscore.SeverityMessage, "message"},
		{etscore.SeveritySkipFile, "skip-file"},
		{etscore.Severity(99), "unknown"},
	}

	for _, tt := range tests {
		t.Run(tt.expected, func(t *testing.T) {
			t.Parallel()
			result := tt.severity.String()
			if result != tt.expected {
				t.Errorf("etscore.Severity(%d).String() = %q, want %q", tt.severity, result, tt.expected)
			}
		})
	}
}

func TestSeverityToCategory(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name     string
		severity etscore.Severity
		expected diagnostics.Category
	}{
		{
			name:     "error maps to CategoryError",
			severity: etscore.SeverityError,
			expected: diagnostics.CategoryError,
		},
		{
			name:     "warning maps to CategoryWarning",
			severity: etscore.SeverityWarning,
			expected: diagnostics.CategoryWarning,
		},
		{
			name:     "suggestion maps to CategorySuggestion",
			severity: etscore.SeveritySuggestion,
			expected: diagnostics.CategorySuggestion,
		},
		{
			name:     "message maps to CategoryMessage",
			severity: etscore.SeverityMessage,
			expected: diagnostics.CategoryMessage,
		},
		{
			name:     "off defaults to CategoryWarning",
			severity: etscore.SeverityOff,
			expected: diagnostics.CategoryWarning,
		},
		{
			name:     "skip-file defaults to CategoryWarning",
			severity: etscore.SeveritySkipFile,
			expected: diagnostics.CategoryWarning,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			result := ToCategory(tt.severity)
			if result != tt.expected {
				t.Errorf("ToCategory(%v) = %v, want %v", tt.severity, result, tt.expected)
			}
		})
	}
}

func TestCollectEffectDirectives(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name     string
		source   string
		expected []Directive
	}{
		{
			name:     "no directives",
			source:   "const x = 1;\nconst y = 2;",
			expected: nil,
		},
		{
			name:   "single directive same line",
			source: "// @effect-diagnostics floatingEffect:off",
			expected: []Directive{
				{Line: 0, IsNextLine: false, Rules: []RuleSeverity{{Rule: "floatingEffect", Severity: etscore.SeverityOff}}},
			},
		},
		{
			name:   "next-line directive",
			source: "// @effect-diagnostics-next-line floatingEffect:off\nEffect.log(\"hello\")",
			expected: []Directive{
				{Line: 0, IsNextLine: true, Rules: []RuleSeverity{{Rule: "floatingEffect", Severity: etscore.SeverityOff}}},
			},
		},
		{
			name:   "multiple rules in one directive",
			source: "// @effect-diagnostics floatingEffect:off pocRule:warning",
			expected: []Directive{
				{Line: 0, IsNextLine: false, Rules: []RuleSeverity{
					{Rule: "floatingEffect", Severity: etscore.SeverityOff},
					{Rule: "pocRule", Severity: etscore.SeverityWarning},
				}},
			},
		},
		{
			name:   "multiple directives on different lines",
			source: "// @effect-diagnostics floatingEffect:off\nconst x = 1;\n// @effect-diagnostics-next-line pocRule:warning",
			expected: []Directive{
				{Line: 0, IsNextLine: false, Rules: []RuleSeverity{{Rule: "floatingEffect", Severity: etscore.SeverityOff}}},
				{Line: 2, IsNextLine: true, Rules: []RuleSeverity{{Rule: "pocRule", Severity: etscore.SeverityWarning}}},
			},
		},
		{
			name:   "skip-file directive",
			source: "// @effect-diagnostics *:skip-file",
			expected: []Directive{
				{Line: 0, IsNextLine: false, Rules: []RuleSeverity{{Rule: "*", Severity: etscore.SeveritySkipFile}}},
			},
		},
		{
			name:   "suggestion severity directive",
			source: "// @effect-diagnostics-next-line floatingEffect:suggestion",
			expected: []Directive{
				{Line: 0, IsNextLine: true, Rules: []RuleSeverity{{Rule: "floatingEffect", Severity: etscore.SeveritySuggestion}}},
			},
		},
		{
			name:   "message severity directive",
			source: "// @effect-diagnostics-next-line floatingEffect:message",
			expected: []Directive{
				{Line: 0, IsNextLine: true, Rules: []RuleSeverity{{Rule: "floatingEffect", Severity: etscore.SeverityMessage}}},
			},
		},
		{
			name:   "all severity levels in one directive",
			source: "// @effect-diagnostics rule1:error rule2:warning rule3:suggestion rule4:message",
			expected: []Directive{
				{Line: 0, IsNextLine: false, Rules: []RuleSeverity{
					{Rule: "rule1", Severity: etscore.SeverityError},
					{Rule: "rule2", Severity: etscore.SeverityWarning},
					{Rule: "rule3", Severity: etscore.SeveritySuggestion},
					{Rule: "rule4", Severity: etscore.SeverityMessage},
				}},
			},
		},
		{
			name:   "wildcard section directive",
			source: "// @effect-diagnostics *:off",
			expected: []Directive{
				{Line: 0, IsNextLine: false, Rules: []RuleSeverity{{Rule: "*", Severity: etscore.SeverityOff}}},
			},
		},
		{
			name:   "wildcard next-line directive",
			source: "// @effect-diagnostics-next-line *:off",
			expected: []Directive{
				{Line: 0, IsNextLine: true, Rules: []RuleSeverity{{Rule: "*", Severity: etscore.SeverityOff}}},
			},
		},
		{
			name:   "mixed wildcard and rule-specific",
			source: "// @effect-diagnostics *:off floatingEffect:error",
			expected: []Directive{
				{Line: 0, IsNextLine: false, Rules: []RuleSeverity{
					{Rule: "*", Severity: etscore.SeverityOff},
					{Rule: "floatingEffect", Severity: etscore.SeverityError},
				}},
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			result := CollectEffectDirectives(tt.source)
			if len(result) != len(tt.expected) {
				t.Errorf("CollectEffectDirectives() returned %d directives, want %d", len(result), len(tt.expected))
				return
			}
			for i, d := range result {
				if d.Line != tt.expected[i].Line {
					t.Errorf("directive[%d].Line = %d, want %d", i, d.Line, tt.expected[i].Line)
				}
				if d.IsNextLine != tt.expected[i].IsNextLine {
					t.Errorf("directive[%d].IsNextLine = %v, want %v", i, d.IsNextLine, tt.expected[i].IsNextLine)
				}
				if len(d.Rules) != len(tt.expected[i].Rules) {
					t.Errorf("directive[%d] has %d rules, want %d", i, len(d.Rules), len(tt.expected[i].Rules))
					continue
				}
				for j, r := range d.Rules {
					if r.Rule != tt.expected[i].Rules[j].Rule {
						t.Errorf("directive[%d].Rules[%d].Rule = %q, want %q", i, j, r.Rule, tt.expected[i].Rules[j].Rule)
					}
					if r.Severity != tt.expected[i].Rules[j].Severity {
						t.Errorf("directive[%d].Rules[%d].Severity = %v, want %v", i, j, r.Severity, tt.expected[i].Rules[j].Severity)
					}
				}
			}
		})
	}
}

func TestDirectiveSetIsSuppressed(t *testing.T) {
	t.Parallel()
	source := `// Line 0
// @effect-diagnostics-next-line floatingEffect:off
Effect.log("hello")  // Line 2
// Normal code       // Line 3`

	directives := CollectEffectDirectives(source)
	ds := BuildDirectiveSet(directives)

	tests := []struct {
		name     string
		ruleName string
		line     int
		expected bool
	}{
		{
			name:     "suppressed by next-line directive",
			ruleName: "floatingEffect",
			line:     2,
			expected: true,
		},
		{
			name:     "not suppressed - different rule",
			ruleName: "pocRule",
			line:     2,
			expected: false,
		},
		{
			name:     "not suppressed - different line",
			ruleName: "floatingEffect",
			line:     3,
			expected: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			result := ds.IsSuppressed(tt.ruleName, tt.line)
			if result != tt.expected {
				t.Errorf("IsSuppressed(%q, %d) = %v, want %v",
					tt.ruleName, tt.line, result, tt.expected)
			}
		})
	}
}

func TestWildcardOffAtTopLineStillLooksSuppressedAtLineZero(t *testing.T) {
	t.Parallel()
	source := `// @effect-diagnostics *:off
// @effect-diagnostics nodeBuiltinImport:warning
import fs from "node:fs"`

	directives := CollectEffectDirectives(source)
	ds := BuildDirectiveSet(directives)

	if !ds.IsSuppressed("*", 0) {
		t.Fatalf("IsSuppressed(*, 0) = false, want true")
	}

	if ds.IsSkipFile("*") {
		t.Fatalf("IsSkipFile(*) = true, want false")
	}
}

func TestWildcardSkipFileIsDistinctFromOff(t *testing.T) {
	t.Parallel()
	source := `// @effect-diagnostics *:skip-file
import fs from "node:fs"`

	directives := CollectEffectDirectives(source)
	ds := BuildDirectiveSet(directives)

	if !ds.IsSkipFile("*") {
		t.Fatalf("IsSkipFile(*) = false, want true")
	}

	if !ds.IsSkipFile("nodeBuiltinImport") {
		t.Fatalf("IsSkipFile(nodeBuiltinImport) = false, want true")
	}
}

func TestHasEnablingDirective(t *testing.T) {
	t.Parallel()
	for _, tt := range []struct {
		name     string
		source   string
		ruleName string
		expected bool
	}{
		{"wildcard error enables floatingEffect", "// @effect-diagnostics *:error\nEffect.succeed(1)", "floatingEffect", true},
		{"wildcard error enables any rule", "// @effect-diagnostics *:error\nEffect.succeed(1)", "someOtherRule", true},
		{"wildcard off does not enable", "// @effect-diagnostics *:off\nEffect.succeed(1)", "floatingEffect", false},
	} {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			ds := BuildDirectiveSet(CollectEffectDirectives(tt.source))
			if got := ds.HasEnablingDirective(tt.ruleName); got != tt.expected {
				t.Errorf("HasEnablingDirective(%q) = %v, want %v", tt.ruleName, got, tt.expected)
			}
		})
	}
}

func TestHasAnyDirectiveForRule(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name     string
		source   string
		ruleName string
		expected bool
	}{
		{
			name:     "no directives",
			source:   "Effect.succeed(1)",
			ruleName: "floatingEffect",
			expected: false,
		},
		{
			name:     "section directive mentions rule by name",
			source:   "// @effect-diagnostics floatingEffect:error\nEffect.succeed(1)",
			ruleName: "floatingEffect",
			expected: true,
		},
		{
			name:     "section directive mentions other rule only",
			source:   "// @effect-diagnostics pocRule:error\nEffect.succeed(1)",
			ruleName: "floatingEffect",
			expected: false,
		},
		{
			name:     "wildcard section directive mentions every rule",
			source:   "// @effect-diagnostics *:error\nEffect.succeed(1)",
			ruleName: "floatingEffect",
			expected: true,
		},
		{
			name:     "lowering directive still counts as a mention",
			source:   "// @effect-diagnostics floatingEffect:off\nEffect.succeed(1)",
			ruleName: "floatingEffect",
			expected: true,
		},
		{
			name:     "next-line directive counts as a mention",
			source:   "// @effect-diagnostics-next-line floatingEffect:off\nEffect.log(\"x\")",
			ruleName: "floatingEffect",
			expected: true,
		},
		{
			name:     "skip-file directive counts as a mention",
			source:   "// @effect-diagnostics floatingEffect:skip-file",
			ruleName: "floatingEffect",
			expected: true,
		},
		{
			name:     "rule name matching is case-insensitive",
			source:   "// @effect-diagnostics FLOATINGeffect:error\nEffect.succeed(1)",
			ruleName: "floatingEffect",
			expected: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			ds := BuildDirectiveSet(CollectEffectDirectives(tt.source))
			if got := ds.HasAnyDirectiveForRule(tt.ruleName); got != tt.expected {
				t.Errorf("HasAnyDirectiveForRule(%q) = %v, want %v", tt.ruleName, got, tt.expected)
			}
		})
	}
}
