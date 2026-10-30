package rules_test

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/rules"
)

func TestEmptyCallbackToNeverVersionDetection(t *testing.T) {
	t.Parallel()
	const source = `// @filename: preview.ts
import { Effect } from "effect"
export const park = Effect.callback<never>(() => {})
`
	for _, test := range []struct {
		name   string
		prefix string
		suffix string
		want   int
	}{
		{"known", "", "", 1},
		{"unknown", unknownEffectVersionPrefix(t), "", 0},
		{"mixed", `// @filename: /node_modules/nested/package.json
{ "name": "nested", "version": "1.0.0", "types": "index.d.ts" }
// @filename: /node_modules/nested/index.d.ts
export { marker } from "effect"
// @filename: /node_modules/nested/node_modules/effect/package.json
{ "name": "effect", "version": "3.19.19", "types": "index.d.ts" }
// @filename: /node_modules/nested/node_modules/effect/index.d.ts
export declare const marker: unique symbol
`, "import { marker } from \"nested\"\nexport const mixedPackage = marker\n", 0},
	} {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			text := test.prefix + source + test.suffix
			if got := len(evaluatePreviewWithoutHooks(t, bundledeffect.EffectV4, text, &rules.EmptyCallbackToNever).Diagnostics); got != test.want {
				t.Fatalf("expected %d emptyCallbackToNever diagnostics, got %d", test.want, got)
			}
		})
	}
}
