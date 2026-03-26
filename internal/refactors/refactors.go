// Package refactors contains all refactor implementations and the registry.
// This mirrors the fixables package structure for code fixes.
package refactors

import "github.com/effect-ts/tsgo/internal/refactor"

// All is the list of all refactor providers.
// Add new refactors here explicitly - no init() magic.
var All = []refactor.Refactor{
	WrapWithPipe,
	TogglePipeStyle,
	PipeableToDatafirst,
	WrapWithEffectGen,
	RemoveUnnecessaryEffectGen,
	AsyncAwaitToGen,
	AsyncAwaitToGenTryPromise,
	AsyncAwaitToFn,
	AsyncAwaitToFnTryPromise,
	EffectGenToFn,
	ToggleTypeAnnotation,
	ToggleReturnTypeAnnotation,
	ToggleLazyConst,
	FunctionToArrow,
	LayerMagic,
	WriteTagClassAccessors,
	TypeToEffectSchema,
	TypeToEffectSchemaClass,
	MakeSchemaOpaque,
	MakeSchemaOpaqueWithNs,
	StructuralTypeToSchema,
}
