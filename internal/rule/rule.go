// Package rule defines the Rule struct for Effect diagnostic rules.
package rule

import (
	"slices"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/microsoft/typescript-go/shim/ast"
)

// Rule defines a diagnostic rule with its metadata and check function.
type Rule struct {
	// Name is the unique identifier used in configuration (e.g., "pocRule", "floatingEffect").
	// This name is used in tsconfig.json diagnosticSeverity config and in @effect-diagnostics directives.
	Name string

	// Group is the diagnostic group this rule belongs to (e.g., "correctness", "antipattern", "effectNative", "style").
	Group string

	// Description explains what the rule checks for (used in documentation/tooling).
	Description string

	// DefaultSeverity is the severity used when the user has not explicitly configured this rule.
	// Every rule must set this field explicitly.
	DefaultSeverity etscore.Severity

	// SupportedEffect lists which Effect versions this rule supports (e.g., "v3", "v4").
	SupportedEffect []string

	// Codes is the list of diagnostic codes that this rule can emit.
	// This is used to build the code-to-rule mapping for codefixes and other lookups.
	Codes []int32

	// Run executes the rule against a source file and returns diagnostics.
	// It should NOT attach diagnostics to the checker - just return them.
	// The hook will handle directive processing, severity transformation, and emission.
	Run func(ctx *Context) []*ast.Diagnostic
}

// ByName finds a rule by name in a slice. Returns nil if not found.
func ByName(rules []Rule, name string) *Rule {
	for i := range rules {
		if rules[i].Name == name {
			return &rules[i]
		}
	}
	return nil
}

// AllCodes returns all diagnostic codes from the given rules.
func AllCodes(rules []Rule) []int32 {
	var codes []int32
	for _, r := range rules {
		codes = append(codes, r.Codes...)
	}
	return codes
}

// CodeToRuleName returns the rule name for a diagnostic code.
// Returns empty string if the code is not found in any rule.
func CodeToRuleName(rules []Rule, code int32) string {
	for _, r := range rules {
		if slices.Contains(r.Codes, code) {
			return r.Name
		}
	}
	return ""
}

// IsEffectCode returns true if code is in the Effect diagnostic range (377000-377999).
func IsEffectCode(code int32) bool {
	return code >= 377000 && code <= 377999
}
