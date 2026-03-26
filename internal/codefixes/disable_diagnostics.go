// Package codefixes provides Effect-specific code fix providers.
package codefixes

import (
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/rules"
)

// EffectDisableErrorCodes returns all Effect diagnostic codes that support disable actions.
func EffectDisableErrorCodes() []int32 {
	return rule.AllCodes(rules.All)
}

// CodeToRuleName returns the rule name for an Effect diagnostic code.
// Returns "unknown" if the code is not recognized.
func CodeToRuleName(code int32) string {
	name := rule.CodeToRuleName(rules.All, code)
	if name == "" {
		return "unknown"
	}
	return name
}

// DisableNextLineComment generates the comment text to disable a rule for the next line.
// Format: // @effect-diagnostics-next-line {ruleName}:off
func DisableNextLineComment(ruleName string) string {
	return "// @effect-diagnostics-next-line " + ruleName + ":off\n"
}

// DisableFileComment generates the comment text to disable a rule for the entire file.
// Format: /** @effect-diagnostics {ruleName}:skip-file */
func DisableFileComment(ruleName string) string {
	return "/** @effect-diagnostics " + ruleName + ":skip-file */\n"
}

// DisableNextLineDescription generates the action description for "Disable for this line".
func DisableNextLineDescription(ruleName string) string {
	return "Disable " + ruleName + " for this line"
}

// DisableFileDescription generates the action description for "Disable for entire file".
func DisableFileDescription(ruleName string) string {
	return "Disable " + ruleName + " for entire file"
}
