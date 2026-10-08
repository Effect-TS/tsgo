package rules

import "testing"

func TestPreferEffectArrayInheritedAugmentation(t *testing.T) {
	t.Parallel()
	ctx, done := compileApiStabilityFiles(t, map[string]string{"test.ts": `
export {};
declare global {
  interface Array<T> extends Pick<ArrayConstructor, "isArray"> {}
}
Array.isArray([]);
`})
	defer done()
	diagnostics := runPreferEffectArray(ctx)
	const want = "Consider effect/Array.isArray instead of native Array.isArray. Narrows elements to unknown rather than any. effect(preferEffectArray)"
	if len(diagnostics) != 1 || diagnostics[0].String() != want {
		t.Fatalf("diagnostics = %v, want one %q", diagnostics, want)
	}
}

func TestPreferEffectArrayRemappedMember(t *testing.T) {
	t.Parallel()
	ctx, done := compileApiStabilityFiles(t, map[string]string{"test.ts": `
type Remapped = { [K in keyof number[] as K extends "filter" ? "map" : never]: number[][K] };
declare const remapped: Remapped;
remapped.map;
`})
	defer done()
	diagnostics := runPreferEffectArray(ctx)
	const want = "Consider effect/Array.filter instead of native Array.filter. The callback type accepts (value, index), with no array parameter or thisArg; sparse-array holes are visited. effect(preferEffectArray)"
	if len(diagnostics) != 1 || diagnostics[0].String() != want {
		t.Fatalf("diagnostics = %v, want one %q", diagnostics, want)
	}
}
