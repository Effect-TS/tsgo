package directives

import (
	"testing"

	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/microsoft/typescript-go/shim/diagnostics"
)

func TestEtscoreParseSeverity(t *testing.T) {
	t.Parallel()
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
		{"unknown", etscore.SeverityError}, // default to error
		{"", etscore.SeverityError},        // default to error
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
		{
			name:   "wildcard skip-file",
			source: "// @effect-diagnostics *:skip-file",
			expected: []Directive{
				{Line: 0, IsNextLine: false, Rules: []RuleSeverity{{Rule: "*", Severity: etscore.SeveritySkipFile}}},
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

func TestDirectiveAffectedLine(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name      string
		directive Directive
		wantLine  int
	}{
		{
			name:      "same line directive",
			directive: Directive{Line: 5, IsNextLine: false},
			wantLine:  5,
		},
		{
			name:      "next line directive",
			directive: Directive{Line: 5, IsNextLine: true},
			wantLine:  6,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			result := tt.directive.AffectedLine()
			if result != tt.wantLine {
				t.Errorf("Directive.AffectedLine() = %d, want %d", result, tt.wantLine)
			}
		})
	}
}

func TestDirectiveSetGetEffectiveSeverity(t *testing.T) {
	t.Parallel()
	source := `// @effect-diagnostics *:skip-file
// Normal code
// @effect-diagnostics-next-line floatingEffect:off
Effect.log("hello")
// @effect-diagnostics pocRule:warning
anotherCall()`

	directives := CollectEffectDirectives(source)
	ds := BuildDirectiveSet(directives)

	tests := []struct {
		name            string
		ruleName        string
		line            int
		defaultSeverity etscore.Severity
		expected        etscore.Severity
	}{
		{
			name:            "file-level skip applies to any rule",
			ruleName:        "anyRule",
			line:            10,
			defaultSeverity: etscore.SeverityError,
			expected:        etscore.SeveritySkipFile,
		},
		{
			name:            "next-line directive affects next line",
			ruleName:        "floatingEffect",
			line:            3, // 0-indexed, so this is the 4th line "Effect.log(\"hello\")"
			defaultSeverity: etscore.SeverityError,
			expected:        etscore.SeveritySkipFile, // file-level takes precedence
		},
		{
			name:            "same-line directive",
			ruleName:        "pocRule",
			line:            4,
			defaultSeverity: etscore.SeverityError,
			expected:        etscore.SeveritySkipFile, // file-level takes precedence
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			result := ds.GetEffectiveSeverity(tt.ruleName, tt.line, tt.defaultSeverity)
			if result != tt.expected {
				t.Errorf("GetEffectiveSeverity(%q, %d, %v) = %v, want %v",
					tt.ruleName, tt.line, tt.defaultSeverity, result, tt.expected)
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

func TestCaseInsensitiveRuleMatching(t *testing.T) {
	t.Parallel()
	source := `// @effect-diagnostics-next-line FloatingEffect:off
Effect.succeed(1)  // Line 1
// @effect-diagnostics-next-line POCRULE:warning
Effect.succeed(2)  // Line 3`

	directives := CollectEffectDirectives(source)
	ds := BuildDirectiveSet(directives)

	tests := []struct {
		name     string
		ruleName string
		line     int
		expected etscore.Severity
	}{
		{
			name:     "lowercase rule matches PascalCase directive",
			ruleName: "floatingeffect",
			line:     1,
			expected: etscore.SeverityOff,
		},
		{
			name:     "PascalCase rule matches PascalCase directive",
			ruleName: "FloatingEffect",
			line:     1,
			expected: etscore.SeverityOff,
		},
		{
			name:     "camelCase rule matches UPPERCASE directive",
			ruleName: "pocRule",
			line:     3,
			expected: etscore.SeverityWarning,
		},
		{
			name:     "UPPERCASE rule matches UPPERCASE directive",
			ruleName: "POCRULE",
			line:     3,
			expected: etscore.SeverityWarning,
		},
		{
			name:     "different rule not affected",
			ruleName: "otherRule",
			line:     1,
			expected: etscore.SeverityError,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			result := ds.GetEffectiveSeverity(tt.ruleName, tt.line, etscore.SeverityError)
			if result != tt.expected {
				t.Errorf("GetEffectiveSeverity(%q, %d, etscore.SeverityError) = %v, want %v",
					tt.ruleName, tt.line, result, tt.expected)
			}
		})
	}
}

func TestSectionDirectives(t *testing.T) {
	t.Parallel()
	source := `// Line 0 - normal code
Effect.succeed(1)  // Line 1 - no suppression
// @effect-diagnostics floatingEffect:off
Effect.succeed(2)  // Line 3 - suppressed by section
Effect.succeed(3)  // Line 4 - still suppressed
// @effect-diagnostics floatingEffect:warning
Effect.succeed(4)  // Line 6 - warning severity
Effect.succeed(5)  // Line 7 - still warning
Effect.succeed(6)  // Line 8 - still warning through EOF`

	directives := CollectEffectDirectives(source)
	ds := BuildDirectiveSet(directives)

	tests := []struct {
		name     string
		line     int
		expected etscore.Severity
	}{
		{
			name:     "before any section directive",
			line:     1,
			expected: etscore.SeverityError,
		},
		{
			name:     "first section directive line",
			line:     3,
			expected: etscore.SeverityOff,
		},
		{
			name:     "within first section",
			line:     4,
			expected: etscore.SeverityOff,
		},
		{
			name:     "second section directive line",
			line:     6,
			expected: etscore.SeverityWarning,
		},
		{
			name:     "within second section",
			line:     7,
			expected: etscore.SeverityWarning,
		},
		{
			name:     "after last section directive (EOF)",
			line:     8,
			expected: etscore.SeverityWarning,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			result := ds.GetEffectiveSeverity("floatingEffect", tt.line, etscore.SeverityError)
			if result != tt.expected {
				t.Errorf("GetEffectiveSeverity(\"floatingEffect\", %d, etscore.SeverityError) = %v, want %v",
					tt.line, result, tt.expected)
			}
		})
	}
}

func TestSectionDirectiveAppliesByRuleUntilOverridden(t *testing.T) {
	t.Parallel()
	source := `// @effect-diagnostics floatingEffect:off
Effect.succeed(1)  // Line 1 - floatingEffect off
// @effect-diagnostics otherRule:warning
Effect.succeed(2)  // Line 3 - floatingEffect still off
Effect.succeed(3)  // Line 4 - floatingEffect still off
// @effect-diagnostics *:error
Effect.succeed(4)  // Line 6 - wildcard override error
Effect.succeed(5)  // Line 7 - wildcard override error`

	directives := CollectEffectDirectives(source)
	ds := BuildDirectiveSet(directives)

	tests := []struct {
		name     string
		ruleName string
		line     int
		expected etscore.Severity
	}{
		{
			name:     "floatingEffect before unrelated section",
			ruleName: "floatingEffect",
			line:     1,
			expected: etscore.SeverityOff,
		},
		{
			name:     "floatingEffect after unrelated section",
			ruleName: "floatingEffect",
			line:     3,
			expected: etscore.SeverityOff,
		},
		{
			name:     "floatingEffect still off before wildcard override",
			ruleName: "floatingEffect",
			line:     4,
			expected: etscore.SeverityOff,
		},
		{
			name:     "otherRule section applies",
			ruleName: "otherRule",
			line:     3,
			expected: etscore.SeverityWarning,
		},
		{
			name:     "wildcard override applies to floatingEffect",
			ruleName: "floatingEffect",
			line:     6,
			expected: etscore.SeverityError,
		},
		{
			name:     "wildcard override applies to otherRule",
			ruleName: "otherRule",
			line:     7,
			expected: etscore.SeverityError,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			result := ds.GetEffectiveSeverity(tt.ruleName, tt.line, etscore.SeverityError)
			if result != tt.expected {
				t.Errorf("GetEffectiveSeverity(%q, %d, etscore.SeverityError) = %v, want %v",
					tt.ruleName, tt.line, result, tt.expected)
			}
		})
	}
}

func TestNextLineDirectiveTakesPrecedenceOverSection(t *testing.T) {
	t.Parallel()
	source := `// @effect-diagnostics floatingEffect:off
Effect.succeed(1)  // Line 1 - suppressed by section
// @effect-diagnostics-next-line floatingEffect:error
Effect.succeed(2)  // Line 3 - error by next-line override
Effect.succeed(3)  // Line 4 - back to off from section`

	directives := CollectEffectDirectives(source)
	ds := BuildDirectiveSet(directives)

	tests := []struct {
		name     string
		line     int
		expected etscore.Severity
	}{
		{
			name:     "section directive applies",
			line:     1,
			expected: etscore.SeverityOff,
		},
		{
			name:     "next-line overrides section",
			line:     3,
			expected: etscore.SeverityError,
		},
		{
			name:     "back to section after next-line",
			line:     4,
			expected: etscore.SeverityOff,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			result := ds.GetEffectiveSeverity("floatingEffect", tt.line, etscore.SeverityError)
			if result != tt.expected {
				t.Errorf("GetEffectiveSeverity(\"floatingEffect\", %d, etscore.SeverityError) = %v, want %v",
					tt.line, result, tt.expected)
			}
		})
	}
}

func TestUsedDirectivesTracking(t *testing.T) {
	t.Parallel()
	source := `// @effect-diagnostics-next-line floatingEffect:off
Effect.succeed(1)  // Line 1 - will be checked
// @effect-diagnostics-next-line pocRule:off
Effect.succeed(2)  // Line 3 - different rule, won't suppress floatingEffect`

	directives := CollectEffectDirectives(source)
	ds := BuildDirectiveSet(directives)

	// Check floatingEffect at line 1 - should suppress and mark used
	sev1 := ds.GetEffectiveSeverityAndMarkUsed("floatingEffect", 1, etscore.SeverityError)
	if sev1 != etscore.SeverityOff {
		t.Errorf("Expected etscore.SeverityOff for floatingEffect at line 1, got %v", sev1)
	}

	// Check floatingEffect at line 3 - pocRule directive shouldn't match
	sev2 := ds.GetEffectiveSeverityAndMarkUsed("floatingEffect", 3, etscore.SeverityError)
	if sev2 != etscore.SeverityError {
		t.Errorf("Expected etscore.SeverityError for floatingEffect at line 3, got %v", sev2)
	}

	// Get unused directives
	unused := ds.GetUnusedNextLineDirectives(directives)
	if len(unused) != 1 {
		t.Errorf("Expected 1 unused directive, got %d", len(unused))
	}
	if len(unused) > 0 && unused[0].Line != 2 {
		t.Errorf("Expected unused directive at line 2, got line %d", unused[0].Line)
	}
}

func TestWildcardSectionPrecedence(t *testing.T) {
	t.Parallel()
	// Mirrors the reference test: floatingEffect_disabledWildcard.ts
	// No directive → *:off suppresses → floatingEffect:error re-enables → *:off suppresses again
	source := `Effect.succeed(1)  // Line 0 - no directive, should fire
// @effect-diagnostics *:off
Effect.succeed(1)  // Line 2 - suppressed by wildcard
// @effect-diagnostics floatingEffect:error
Effect.succeed(1)  // Line 4 - re-enabled by rule-specific
// @effect-diagnostics *:off
Effect.succeed(1)  // Line 6 - suppressed again by wildcard`

	directives := CollectEffectDirectives(source)
	ds := BuildDirectiveSet(directives)

	tests := []struct {
		name     string
		line     int
		expected etscore.Severity
	}{
		{
			name:     "before any directive - default severity",
			line:     0,
			expected: etscore.SeverityError,
		},
		{
			name:     "after *:off - suppressed",
			line:     2,
			expected: etscore.SeverityOff,
		},
		{
			name:     "after floatingEffect:error - re-enabled",
			line:     4,
			expected: etscore.SeverityError,
		},
		{
			name:     "after second *:off - suppressed again",
			line:     6,
			expected: etscore.SeverityOff,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			result := ds.GetEffectiveSeverity("floatingEffect", tt.line, etscore.SeverityError)
			if result != tt.expected {
				t.Errorf("GetEffectiveSeverity(\"floatingEffect\", %d, etscore.SeverityError) = %v, want %v",
					tt.line, result, tt.expected)
			}
		})
	}
}

func TestWildcardNextLineSuppression(t *testing.T) {
	t.Parallel()
	source := `// @effect-diagnostics-next-line *:off
Effect.succeed(1)  // Line 1 - suppressed by wildcard next-line
Effect.succeed(2)  // Line 2 - not suppressed`

	directives := CollectEffectDirectives(source)
	ds := BuildDirectiveSet(directives)

	tests := []struct {
		name     string
		ruleName string
		line     int
		expected etscore.Severity
	}{
		{
			name:     "wildcard suppresses floatingEffect",
			ruleName: "floatingEffect",
			line:     1,
			expected: etscore.SeverityOff,
		},
		{
			name:     "wildcard suppresses any rule",
			ruleName: "someOtherRule",
			line:     1,
			expected: etscore.SeverityOff,
		},
		{
			name:     "next line not affected",
			ruleName: "floatingEffect",
			line:     2,
			expected: etscore.SeverityError,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			result := ds.GetEffectiveSeverity(tt.ruleName, tt.line, etscore.SeverityError)
			if result != tt.expected {
				t.Errorf("GetEffectiveSeverity(%q, %d, etscore.SeverityError) = %v, want %v",
					tt.ruleName, tt.line, result, tt.expected)
			}
		})
	}
}

func TestWildcardSkipFile(t *testing.T) {
	t.Parallel()
	source := `// @effect-diagnostics *:skip-file
Effect.succeed(1)  // Line 1 - skipped`

	directives := CollectEffectDirectives(source)
	ds := BuildDirectiveSet(directives)

	tests := []struct {
		name     string
		ruleName string
	}{
		{"skips floatingEffect", "floatingEffect"},
		{"skips any rule", "anyOtherRule"},
		{"skips pocRule", "pocRule"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			result := ds.GetEffectiveSeverity(tt.ruleName, 1, etscore.SeverityError)
			if result != etscore.SeveritySkipFile {
				t.Errorf("GetEffectiveSeverity(%q, 1, etscore.SeverityError) = %v, want etscore.SeveritySkipFile",
					tt.ruleName, result)
			}
		})
	}
}

func TestWildcardUnusedDirectiveTracking(t *testing.T) {
	t.Parallel()
	source := `// @effect-diagnostics-next-line *:off
Effect.succeed(1)  // Line 1 - wildcard next-line
// @effect-diagnostics-next-line floatingEffect:off
Effect.succeed(2)  // Line 3 - specific rule next-line`

	directives := CollectEffectDirectives(source)
	ds := BuildDirectiveSet(directives)

	// Don't mark any directive as used - neither should suppress a diagnostic
	// The wildcard directive should still NOT appear as unused
	unused := ds.GetUnusedNextLineDirectives(directives)

	// Only the specific rule directive should be reported as unused, not the wildcard
	if len(unused) != 1 {
		t.Fatalf("Expected 1 unused directive, got %d", len(unused))
	}
	if unused[0].Line != 2 {
		t.Errorf("Expected unused directive at line 2, got line %d", unused[0].Line)
	}
	// Verify it's the floatingEffect one, not the wildcard
	if unused[0].Rules[0].Rule != "floatingEffect" {
		t.Errorf("Expected unused directive for floatingEffect, got %q", unused[0].Rules[0].Rule)
	}
}

func TestHasEnablingDirectiveWithWildcard(t *testing.T) {
	t.Parallel()
	source := `// @effect-diagnostics *:error
Effect.succeed(1)`

	directives := CollectEffectDirectives(source)
	ds := BuildDirectiveSet(directives)

	tests := []struct {
		name     string
		ruleName string
		expected bool
	}{
		{
			name:     "wildcard enables floatingEffect",
			ruleName: "floatingEffect",
			expected: true,
		},
		{
			name:     "wildcard enables any rule",
			ruleName: "someOtherRule",
			expected: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			result := ds.HasEnablingDirective(tt.ruleName)
			if result != tt.expected {
				t.Errorf("HasEnablingDirective(%q) = %v, want %v", tt.ruleName, result, tt.expected)
			}
		})
	}
}

func TestHasEnablingDirectiveWithWildcardOff(t *testing.T) {
	t.Parallel()
	source := `// @effect-diagnostics *:off
Effect.succeed(1)`

	directives := CollectEffectDirectives(source)
	ds := BuildDirectiveSet(directives)

	// *:off should NOT count as enabling
	result := ds.HasEnablingDirective("floatingEffect")
	if result != false {
		t.Errorf("HasEnablingDirective(\"floatingEffect\") = %v, want false (wildcard off should not enable)", result)
	}
}
