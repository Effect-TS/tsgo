// Package etscore provides core types for Effect TypeScript integration.
// This is a leaf package with no dependencies, designed to be imported by
// both typescript-go/internal/core and effectapi without circular imports.
package etscore

import "strings"

// Severity represents the configured severity level for a diagnostic rule.
type Severity int

const (
	SeverityOff Severity = iota
	SeverityWarning
	SeverityError
	SeveritySuggestion
	SeverityMessage
	SeveritySkipFile // Used in directives only, not in tsconfig
)

// ParseSeverity parses a severity string into a Severity constant.
// Valid values: "off", "warning", "warn", "error", "suggestion", "message", "skip-file"
func ParseSeverity(s string) Severity {
	switch strings.ToLower(strings.TrimSpace(s)) {
	case "off":
		return SeverityOff
	case "warning", "warn":
		return SeverityWarning
	case "error":
		return SeverityError
	case "suggestion":
		return SeveritySuggestion
	case "message":
		return SeverityMessage
	case "skip-file":
		return SeveritySkipFile
	default:
		return SeverityError // default to error for unknown values
	}
}

// String returns the string representation of a Severity.
func (s Severity) String() string {
	switch s {
	case SeverityOff:
		return "off"
	case SeverityWarning:
		return "warning"
	case SeverityError:
		return "error"
	case SeveritySuggestion:
		return "suggestion"
	case SeverityMessage:
		return "message"
	case SeveritySkipFile:
		return "skip-file"
	default:
		return "unknown"
	}
}

// IsOff returns true if the severity disables the diagnostic.
func (s Severity) IsOff() bool {
	return s == SeverityOff || s == SeveritySkipFile
}

// visibilityRank orders severities by how prominently they surface in output:
// error > warning > suggestion > message > off/skip-file. This is distinct
// from the declaration order of the constants, which is not a visibility
// ordering.
func (s Severity) visibilityRank() int {
	switch s {
	case SeverityError:
		return 4
	case SeverityWarning:
		return 3
	case SeveritySuggestion:
		return 2
	case SeverityMessage:
		return 1
	default:
		return 0
	}
}

// AtLeastAsVisibleAs reports whether s surfaces at least as prominently as
// min. Off and skip-file severities are never at least as visible as any
// enabled severity.
func (s Severity) AtLeastAsVisibleAs(min Severity) bool {
	return s.visibilityRank() >= min.visibilityRank()
}

// MarshalJSON implements json.Marshaler for Severity.
// Serializes as the string representation (e.g., "error", "warning").
func (s Severity) MarshalJSON() ([]byte, error) {
	return []byte(`"` + s.String() + `"`), nil
}

// UnmarshalJSON implements json.Unmarshaler for Severity.
// Parses from string representation (e.g., "error", "warning").
func (s *Severity) UnmarshalJSON(data []byte) error {
	// Remove quotes from JSON string
	str := strings.Trim(string(data), `"`)
	*s = ParseSeverity(str)
	return nil
}
