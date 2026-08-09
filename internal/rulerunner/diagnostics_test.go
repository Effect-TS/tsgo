package rulerunner

import (
	"testing"

	"github.com/effect-ts/tsgo/etscore"
)

// Not parallel: EnterCommandLineMode toggles process-global state.
func TestMinVisibleSeverity(t *testing.T) { //nolint:paralleltest
	includeSuggestions := &etscore.EffectPluginOptions{IncludeSuggestionsInTsc: true}
	dropSuggestions := &etscore.EffectPluginOptions{IncludeSuggestionsInTsc: false}

	t.Run("outside CLI mode every severity is visible", func(t *testing.T) {
		if got := MinVisibleSeverity(dropSuggestions); got != etscore.SeverityMessage {
			t.Errorf("MinVisibleSeverity = %v, want %v", got, etscore.SeverityMessage)
		}
	})

	t.Run("CLI mode without includeSuggestionsInTsc only surfaces warnings and errors", func(t *testing.T) {
		restore := etscore.EnterCommandLineMode()
		defer restore()
		if got := MinVisibleSeverity(dropSuggestions); got != etscore.SeverityWarning {
			t.Errorf("MinVisibleSeverity = %v, want %v", got, etscore.SeverityWarning)
		}
	})

	t.Run("CLI mode with includeSuggestionsInTsc keeps every severity", func(t *testing.T) {
		restore := etscore.EnterCommandLineMode()
		defer restore()
		if got := MinVisibleSeverity(includeSuggestions); got != etscore.SeverityMessage {
			t.Errorf("MinVisibleSeverity = %v, want %v", got, etscore.SeverityMessage)
		}
	})

	t.Run("CLI mode with nil config keeps every severity", func(t *testing.T) {
		restore := etscore.EnterCommandLineMode()
		defer restore()
		if got := MinVisibleSeverity(nil); got != etscore.SeverityMessage {
			t.Errorf("MinVisibleSeverity = %v, want %v", got, etscore.SeverityMessage)
		}
	})
}
