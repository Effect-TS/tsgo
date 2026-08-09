package etscore

import "testing"

func TestSeverityAtLeastAsVisibleAs(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name     string
		severity Severity
		min      Severity
		expected bool
	}{
		{"error meets warning threshold", SeverityError, SeverityWarning, true},
		{"warning meets warning threshold", SeverityWarning, SeverityWarning, true},
		{"suggestion below warning threshold", SeveritySuggestion, SeverityWarning, false},
		{"message below warning threshold", SeverityMessage, SeverityWarning, false},
		{"off below warning threshold", SeverityOff, SeverityWarning, false},
		{"skip-file below warning threshold", SeveritySkipFile, SeverityWarning, false},
		{"error meets message threshold", SeverityError, SeverityMessage, true},
		{"warning meets message threshold", SeverityWarning, SeverityMessage, true},
		{"suggestion meets message threshold", SeveritySuggestion, SeverityMessage, true},
		{"message meets message threshold", SeverityMessage, SeverityMessage, true},
		{"off below message threshold", SeverityOff, SeverityMessage, false},
		{"suggestion below error threshold", SeveritySuggestion, SeverityError, false},
		{"warning below error threshold", SeverityWarning, SeverityError, false},
		{"error meets error threshold", SeverityError, SeverityError, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			if got := tt.severity.AtLeastAsVisibleAs(tt.min); got != tt.expected {
				t.Errorf("%v.AtLeastAsVisibleAs(%v) = %v, want %v", tt.severity, tt.min, got, tt.expected)
			}
		})
	}
}
