// Package directives implements parsing for @effect-diagnostics directive comments.
package directives

import (
	"regexp"
	"strings"

	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/microsoft/typescript-go/shim/diagnostics"
)

// ToCategory converts a Severity to the corresponding diagnostics.Category for diagnostic reporting.
// This function stays here because etscore cannot import diagnostics (leaf package).
// etscore.SeverityOff and etscore.SeveritySkipFile return CategoryWarning as a fallback (callers should
// check for these and skip reporting entirely).
func ToCategory(s etscore.Severity) diagnostics.Category {
	switch s {
	case etscore.SeverityError:
		return diagnostics.CategoryError
	case etscore.SeverityWarning:
		return diagnostics.CategoryWarning
	case etscore.SeveritySuggestion:
		return diagnostics.CategorySuggestion
	case etscore.SeverityMessage:
		return diagnostics.CategoryMessage
	default:
		// Off and SkipFile shouldn't reach diagnostic creation,
		// but return Warning as a safe default
		return diagnostics.CategoryWarning
	}
}

// RuleSeverity represents a rule:severity pair from a directive.
type RuleSeverity struct {
	Rule     string
	Severity etscore.Severity
}

// Directive represents a parsed @effect-diagnostics directive.
type Directive struct {
	// Line is the 0-indexed line number where the directive appears
	Line int
	// Pos is the start position of the directive in the source text
	Pos int
	// End is the end position of the directive in the source text
	End int
	// IsNextLine is true if this is a -next-line directive (affects line+1)
	IsNextLine bool
	// Rules is the list of rule:severity pairs
	Rules []RuleSeverity
}

// AffectedLine returns the line number that this directive affects.
// For regular directives, it affects the same line.
// For -next-line directives, it affects the following line.
func (d *Directive) AffectedLine() int {
	if d.IsNextLine {
		return d.Line + 1
	}
	return d.Line
}

// directivePattern matches @effect-diagnostics(-next-line)? followed by rule:level pairs
// Examples:
//   - @effect-diagnostics floatingEffect:off
//   - @effect-diagnostics-next-line floatingEffect:off
//   - @effect-diagnostics floatingEffect:off pocRule:warning
//   - @effect-diagnostics *:skip-file
var directivePattern = regexp.MustCompile(`@effect-diagnostics(-next-line)?\s+([\w:\-*]+(?:\s+[\w:\-*]+)*)`)

// ruleSeverityPattern matches a single rule:severity pair
var ruleSeverityPattern = regexp.MustCompile(`(\w+|\*):(\w+(?:-\w+)?)`)

// CollectEffectDirectives scans the source text and returns all Effect directives found.
func CollectEffectDirectives(sourceText string) []Directive {
	lines := strings.Split(sourceText, "\n")
	var directives []Directive
	lineStartPos := 0

	for lineNum, line := range lines {
		// Use FindStringSubmatchIndex to get positions within the line
		matchIndices := directivePattern.FindStringSubmatchIndex(line)
		if matchIndices == nil {
			lineStartPos += len(line) + 1 // +1 for newline
			continue
		}

		// matchIndices[0:2] is the full match range
		matchStart := matchIndices[0]
		matchEnd := matchIndices[1]

		// matchIndices[2:4] is the first capture group (-next-line or empty)
		isNextLine := matchIndices[2] != -1 && line[matchIndices[2]:matchIndices[3]] == "-next-line"

		// matchIndices[4:6] is the second capture group (rules string)
		rulesStr := line[matchIndices[4]:matchIndices[5]]

		var rules []RuleSeverity
		rulePairs := ruleSeverityPattern.FindAllStringSubmatch(rulesStr, -1)
		for _, pair := range rulePairs {
			if len(pair) == 3 {
				rules = append(rules, RuleSeverity{
					Rule:     pair[1],
					Severity: etscore.ParseSeverity(pair[2]),
				})
			}
		}

		if len(rules) > 0 {
			directives = append(directives, Directive{
				Line:       lineNum,
				Pos:        lineStartPos + matchStart,
				End:        lineStartPos + matchEnd,
				IsNextLine: isNextLine,
				Rules:      rules,
			})
		}

		lineStartPos += len(line) + 1 // +1 for newline
	}

	return directives
}

// DirectiveSet provides efficient lookup of directives by line and rule.
type DirectiveSet struct {
	// byLine maps affected line number to directives affecting that line (for next-line directives)
	byLine map[int][]Directive
	// fileLevel contains file-level suppressions (skip-file directives)
	fileLevel []RuleSeverity
	// sectionDirectives contains non-next-line, non-skip-file directives sorted by line
	// These apply from their position forward until the next section directive or end of file
	sectionDirectives []Directive
	// usedDirectives tracks which directive lines have successfully suppressed a diagnostic
	// Key is the directive line number (not the affected line)
	usedDirectives map[int]bool
}

// BuildDirectiveSet creates a DirectiveSet from a slice of directives for efficient lookup.
func BuildDirectiveSet(directives []Directive) *DirectiveSet {
	ds := &DirectiveSet{
		byLine:         make(map[int][]Directive),
		usedDirectives: make(map[int]bool),
	}

	for _, d := range directives {
		hasSkipFile := false
		// Check for file-level skip-file directives
		for _, rs := range d.Rules {
			if rs.Severity == etscore.SeveritySkipFile {
				ds.fileLevel = append(ds.fileLevel, rs)
				hasSkipFile = true
			}
		}

		if d.IsNextLine {
			// Next-line directives affect a specific line
			affectedLine := d.AffectedLine()
			ds.byLine[affectedLine] = append(ds.byLine[affectedLine], d)
		} else if !hasSkipFile {
			// Section directives (non-next-line, non-skip-file) apply from their position forward
			// Filter out skip-file rules since they're already in fileLevel
			sectionRules := make([]RuleSeverity, 0, len(d.Rules))
			for _, rs := range d.Rules {
				if rs.Severity != etscore.SeveritySkipFile {
					sectionRules = append(sectionRules, rs)
				}
			}
			if len(sectionRules) > 0 {
				sectionDirective := Directive{
					Line:       d.Line,
					IsNextLine: false,
					Rules:      sectionRules,
				}
				ds.sectionDirectives = append(ds.sectionDirectives, sectionDirective)
			}
		}
	}

	return ds
}

// GetEffectiveSeverity returns the effective severity for a rule at a given line.
// It checks file-level suppressions first, then next-line directives, then section directives.
// If no directive applies, it returns the provided default severity.
// Rule matching is case-insensitive.
func (ds *DirectiveSet) GetEffectiveSeverity(ruleName string, line int, defaultSeverity etscore.Severity) etscore.Severity {
	ruleLower := strings.ToLower(ruleName)

	// Check file-level suppressions first (highest priority)
	for _, rs := range ds.fileLevel {
		ruleNameLower := strings.ToLower(rs.Rule)
		if ruleNameLower == ruleLower || ruleNameLower == "*" {
			return etscore.SeveritySkipFile
		}
	}

	// Check next-line directives for this specific line
	if directives, ok := ds.byLine[line]; ok {
		for _, d := range directives {
			for _, rs := range d.Rules {
				ruleNameLower := strings.ToLower(rs.Rule)
				if ruleNameLower == ruleLower || ruleNameLower == "*" {
					return rs.Severity
				}
			}
		}
	}

	// Check section directives (find the most recent one for this rule before this line)
	for i := len(ds.sectionDirectives) - 1; i >= 0; i-- {
		sd := ds.sectionDirectives[i]
		if sd.Line > line {
			continue
		}
		for _, rs := range sd.Rules {
			ruleNameLower := strings.ToLower(rs.Rule)
			if ruleNameLower == ruleLower || ruleNameLower == "*" {
				return rs.Severity
			}
		}
	}

	return defaultSeverity
}

// IsSuppressed returns true if the given rule is suppressed (off or skip-file) at the given line.
func (ds *DirectiveSet) IsSuppressed(ruleName string, line int) bool {
	severity := ds.GetEffectiveSeverity(ruleName, line, etscore.SeverityError)
	return severity == etscore.SeverityOff || severity == etscore.SeveritySkipFile
}

// IsSkipFile returns true if a skip-file directive applies to the given rule.
// Unlike IsSuppressed, this only considers true file-level skip-file directives.
func (ds *DirectiveSet) IsSkipFile(ruleName string) bool {
	ruleLower := strings.ToLower(ruleName)
	for _, rs := range ds.fileLevel {
		ruleNameLower := strings.ToLower(rs.Rule)
		if ruleNameLower == ruleLower || ruleNameLower == "*" {
			return true
		}
	}
	return false
}

// GetEffectiveSeverityAndMarkUsed returns the effective severity and marks the directive as used
// if it affected the result. This is used to track unused directives for warnings.
func (ds *DirectiveSet) GetEffectiveSeverityAndMarkUsed(ruleName string, line int, defaultSeverity etscore.Severity) etscore.Severity {
	ruleLower := strings.ToLower(ruleName)

	// Check file-level suppressions first (highest priority)
	// File-level skip-file directives are never marked as "unused" since they affect the whole file
	for _, rs := range ds.fileLevel {
		ruleNameLower := strings.ToLower(rs.Rule)
		if ruleNameLower == ruleLower || ruleNameLower == "*" {
			return etscore.SeveritySkipFile
		}
	}

	// Check next-line directives for this specific line
	if directives, ok := ds.byLine[line]; ok {
		for _, d := range directives {
			for _, rs := range d.Rules {
				ruleNameLower := strings.ToLower(rs.Rule)
				if ruleNameLower == ruleLower || ruleNameLower == "*" {
					// Mark this directive as used
					ds.usedDirectives[d.Line] = true
					return rs.Severity
				}
			}
		}
	}

	// Check section directives (find the most recent one for this rule before this line)
	for i := len(ds.sectionDirectives) - 1; i >= 0; i-- {
		sd := ds.sectionDirectives[i]
		if sd.Line > line {
			continue
		}
		for _, rs := range sd.Rules {
			ruleNameLower := strings.ToLower(rs.Rule)
			if ruleNameLower == ruleLower || ruleNameLower == "*" {
				// Section directives are not marked as unused since they affect multiple lines
				return rs.Severity
			}
		}
	}

	return defaultSeverity
}

// HasEnablingDirective returns true if any file-level section directive or next-line directive
// enables the given rule (i.e., sets it to a non-off, non-skip-file severity).
// This is used to avoid skipping rules that are off by default but enabled via directives.
func (ds *DirectiveSet) HasEnablingDirective(ruleName string) bool {
	ruleLower := strings.ToLower(ruleName)

	// Check section directives
	for _, sd := range ds.sectionDirectives {
		for _, rs := range sd.Rules {
			ruleNameLower := strings.ToLower(rs.Rule)
			if (ruleNameLower == ruleLower || ruleNameLower == "*") && !rs.Severity.IsOff() {
				return true
			}
		}
	}

	// Check next-line directives
	for _, directives := range ds.byLine {
		for _, d := range directives {
			for _, rs := range d.Rules {
				ruleNameLower := strings.ToLower(rs.Rule)
				if (ruleNameLower == ruleLower || ruleNameLower == "*") && !rs.Severity.IsOff() {
					return true
				}
			}
		}
	}

	return false
}

// GetUnusedNextLineDirectives returns next-line directives that did not suppress any diagnostic.
// This is used to report unusedDirective warnings.
func (ds *DirectiveSet) GetUnusedNextLineDirectives(allDirectives []Directive) []Directive {
	var unused []Directive
	for _, d := range allDirectives {
		if d.IsNextLine && !ds.usedDirectives[d.Line] {
			// Skip wildcard next-line directives from unused tracking since they apply to all rules.
			hasWildcard := false
			for _, rs := range d.Rules {
				if rs.Rule == "*" {
					hasWildcard = true
					break
				}
			}
			if hasWildcard {
				continue
			}
			unused = append(unused, d)
		}
	}
	return unused
}
