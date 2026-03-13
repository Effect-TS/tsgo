// Package completions contains all completion implementations and the registry.
// This mirrors the fixables/refactors package structure.
package completions

import "github.com/effect-ts/effect-typescript-go/internal/completion"

// All is the list of all completion providers.
// Add new completions here explicitly - no init() magic.
var All = []completion.Completion{
	effectSchemaSelfInClasses,
	effectDataClasses,
	serviceMapSelfInClasses,
	genFunctionStar,
	fnFunctionStar,
	effectDiagnosticsComment,
	effectCodegensComment,
	effectJsdocComment,
	durationInput,
	contextSelfInClasses,
	effectSelfInClasses,
	effectSqlModelSelfInClasses,
	rpcMakeClasses,
	schemaBrand,
}

// ByName finds a completion by its unique name.
func ByName(name string) *completion.Completion {
	for i := range All {
		if All[i].Name == name {
			return &All[i]
		}
	}
	return nil
}
