// Package completion defines the Completion type for custom completion providers.
// This mirrors the rule/fixable/refactor pattern — Completion defines what a custom
// completion is, while individual implementations live in internal/completions.
package completion

import (
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

// Completion defines a custom completion provider.
// Each completion has a name, description, and a Run function that returns
// additional completion items to merge into the existing completion list.
type Completion struct {
	// Name is the unique identifier for this completion (e.g., "genFunctionStar").
	Name string

	// Description explains what this completion provides (used in documentation/tooling).
	Description string

	// Run executes the completion and returns additional completion items.
	// The Context provides access to the source file, cursor position,
	// existing completion items, and type checker.
	// Return nil or empty slice if no custom completions are applicable.
	Run func(ctx *Context) []*lsproto.CompletionItem
}
