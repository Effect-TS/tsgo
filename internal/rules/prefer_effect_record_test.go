package rules

import "testing"

func TestPreferEffectRecordInheritedAugmentation(t *testing.T) {
	t.Parallel()
	ctx, done := compileApiStabilityFiles(t, map[string]string{"test.ts": `
export {};
declare global { interface Object extends Pick<ObjectConstructor, "keys"> {} }
Object.keys({ a: 1 });
`})
	defer done()
	diagnostics := runPreferEffectRecord(ctx)
	const want = "Consider effect/Record.keys instead of native Object.keys. Requires a record rather than arrays or primitives; types keys as the inferred key union, which does not guarantee exact runtime keys. effect(preferEffectRecord)"
	if len(diagnostics) != 1 || diagnostics[0].String() != want {
		t.Fatalf("diagnostics = %v, want one %q", diagnostics, want)
	}
}

func TestPreferEffectRecordRemappedMember(t *testing.T) {
	t.Parallel()
	ctx, done := compileApiStabilityFiles(t, map[string]string{"test.ts": `
type Remapped = { [K in keyof ObjectConstructor as K extends "values" ? "keys" : never]: ObjectConstructor[K] };
declare const remapped: Remapped;
remapped.keys;
`})
	defer done()
	diagnostics := runPreferEffectRecord(ctx)
	const want = "Consider effect/Record.values instead of native Object.values. Requires a record rather than arrays or primitives; snapshots keys before reads, so getter or proxy mutations can change the result. effect(preferEffectRecord)"
	if len(diagnostics) != 1 || diagnostics[0].String() != want {
		t.Fatalf("diagnostics = %v, want one %q", diagnostics, want)
	}
}
