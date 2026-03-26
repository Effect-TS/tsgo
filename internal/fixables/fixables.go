// Package fixables contains all code fix implementations and the registry.
// This mirrors the rules package structure for diagnostics.
package fixables

import "github.com/effect-ts/tsgo/internal/fixable"

// All is the list of all code fix providers.
// Add new fixables here explicitly - no init() magic.
var All = []fixable.Fixable{
	// Disable actions (handles all Effect diagnostic codes)
	EffectDisable,
	// Add future fixables here:
	// POCRuleFix,
	// FloatingEffectYieldFix,
	MissingReturnYieldStarFix,
	MissingStarInYieldEffectGenFix,
	CatchAllToMapErrorFix,
	EffectFnIifeFix,
	UnnecessaryPipeFix,
	UnnecessaryPipeChainFix,
	ReturnEffectInGenFix,
	EffectSucceedWithVoidFix,
	UnnecessaryEffectGenFix,
	EffectMapVoidFix,
	UnnecessaryFailYieldableErrorFix,
	ClassSelfMismatchFix,
	EffectFnOpportunityFix,
	DeterministicKeysFix,
	ScopeInLayerEffectScopedFix,
	OverriddenSchemaConstructorFix,
	InstanceOfSchemaFix,
	LayerMergeAllWithDependenciesFix,
	MissingEffectErrorCatchFix,
	MultipleEffectProvideFix,
	SchemaStructWithTagFix,
	RedundantSchemaTagIdentifierRemoveIdentifierFix,
	RunEffectInsideEffectFix,
	SchemaUnionOfLiteralsFix,
	MissedPipeableOpportunityFix,
	ServiceNotAsClassFix,
}

// ByErrorCode finds all fixables that handle a given error code.
func ByErrorCode(code int32) []*fixable.Fixable {
	var results []*fixable.Fixable
	for i := range All {
		if All[i].HandlesCode(code) {
			results = append(results, &All[i])
		}
	}
	return results
}

// AllErrorCodes returns all unique error codes handled by any fixable.
func AllErrorCodes() []int32 {
	seen := make(map[int32]bool)
	var codes []int32
	for _, f := range All {
		for _, code := range f.ErrorCodes {
			if !seen[code] {
				seen[code] = true
				codes = append(codes, code)
			}
		}
	}
	return codes
}

// AllFixIDs returns all unique fix IDs from all fixables.
func AllFixIDs() []string {
	seen := make(map[string]bool)
	var ids []string
	for _, f := range All {
		for _, id := range f.FixIDs {
			if !seen[id] {
				seen[id] = true
				ids = append(ids, id)
			}
		}
	}
	return ids
}
