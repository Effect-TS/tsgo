package rulerunner

import (
	"testing"

	"github.com/effect-ts/tsgo/etscore"
)

func TestMinVisibleSeverity(t *testing.T) { //nolint:paralleltest
	cases := []struct {
		name    string
		cli     bool
		options *etscore.EffectPluginOptions
		want    etscore.Severity
	}{
		{name: "outside CLI mode every severity is visible", options: &etscore.EffectPluginOptions{IncludeSuggestionsInTsc: false}, want: etscore.SeverityMessage},
		{name: "CLI mode without includeSuggestionsInTsc only surfaces warnings and errors", cli: true, options: &etscore.EffectPluginOptions{IncludeSuggestionsInTsc: false}, want: etscore.SeverityWarning},
		{name: "CLI mode with includeSuggestionsInTsc keeps every severity", cli: true, options: &etscore.EffectPluginOptions{IncludeSuggestionsInTsc: true}, want: etscore.SeverityMessage},
		{name: "CLI mode with nil config keeps every severity", cli: true, want: etscore.SeverityMessage},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if tc.cli {
				defer etscore.EnterCommandLineMode()()
			}
			if got := MinVisibleSeverity(tc.options); got != tc.want {
				t.Errorf("MinVisibleSeverity = %v, want %v", got, tc.want)
			}
		})
	}
}
