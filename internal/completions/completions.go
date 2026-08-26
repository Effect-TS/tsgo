// Package completions contains all completion implementations and the registry.
// This mirrors the fixables/refactors package structure.
package completions

import "github.com/effect-ts/tsgo/internal/completion"

// All is the list of all completion providers.
// Add new completions here explicitly - no init() magic.
var All = []completion.Completion{
	effectSchemaSelfInClasses,
	effectDataClasses,
	contextSelfInClasses,
	genFunctionStar,
	fnFunctionStar,
	effectDiagnosticsComment,
	effectCodegensComment,
	effectJsdocComment,
	durationInput,
	effectSelfInClasses,
	effectSqlModelSelfInClasses,
	rpcMakeClasses,
	schemaBrand,
}
