// Package refactor defines the Refactor type for selection-based refactoring providers.
// This mirrors the fixable.Fixable pattern — Refactor defines what a refactor is,
// while individual refactor implementations live in internal/refactors.
package refactor

import (
	"github.com/microsoft/typescript-go/shim/ls"
)

// Refactor defines a selection-based refactoring action.
// Unlike Fixable, refactors are not tied to a specific diagnostic error code —
// they apply based on the user's selection.
type Refactor struct {
	// Name is the unique identifier for this refactor (e.g., "wrapWithPipe").
	Name string

	// Description explains what this refactor does (shown in the editor's refactor menu).
	Description string

	// Kind is the CodeActionKind suffix (e.g., "rewrite.effect.wrapWithPipe").
	Kind string

	// Run executes the refactor and returns code actions for the given context.
	// The Context provides access to the source file, span, type checker,
	// and tracker lifecycle helpers.
	// Return nil or empty slice if the refactor is not applicable.
	Run func(ctx *Context) []ls.CodeAction
}
