// Package fixable defines the Fixable type for code fix providers.
// This mirrors the Rule pattern for diagnostics - Fixable defines what a code fix is,
// while individual fixable implementations live in internal/fixables.
package fixable

import (
	"slices"

	"github.com/microsoft/typescript-go/shim/ls"
)

// Fixable defines a code fix provider for a set of diagnostic codes.
// This is analogous to how Rule defines a diagnostic provider.
type Fixable struct {
	// Name is the unique identifier for this fixable (e.g., "effectDisable", "floatingEffectYield").
	Name string

	// Description explains what this fixable does (used in documentation/tooling).
	Description string

	// ErrorCodes is the set of diagnostic codes this fixable handles.
	// A fixable can handle multiple related error codes.
	ErrorCodes []int32

	// FixIDs for "fix all" support (optional). Each fix action can have its own ID.
	FixIDs []string

	// Run executes the fixable and returns code actions for the given context.
	// The Context provides access to the source file, span, error code,
	// type checker, and tracker lifecycle helpers.
	// Return nil or empty slice if no fixes are applicable.
	Run func(ctx *Context) []ls.CodeAction
}

// HandlesCode returns true if this fixable handles the given error code.
func (f *Fixable) HandlesCode(code int32) bool {
	return slices.Contains(f.ErrorCodes, code)
}
