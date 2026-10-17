package rules_test

import (
	"encoding/json"
	"testing"
	"testing/fstest"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/rules"
)

func TestEffectSleepZeroToYieldNowVersionDetection(t *testing.T) {
	t.Parallel()
	testfs := make(map[string]any)
	if err := bundledeffect.MountEffect(bundledeffect.EffectV4, testfs); err != nil {
		t.Fatal(err)
	}
	packageFile := testfs["/node_modules/effect/package.json"].(*fstest.MapFile)
	var packageJSON map[string]json.RawMessage
	if err := json.Unmarshal(packageFile.Data, &packageJSON); err != nil {
		t.Fatal(err)
	}
	delete(packageJSON, "version")
	unknownPackage, err := json.Marshal(packageJSON)
	if err != nil {
		t.Fatal(err)
	}
	const source = `// @filename: preview.ts
import { Effect } from "effect"
export const sleep = Effect.sleep(0)
`
	for _, test := range []struct {
		name   string
		source string
		want   int
	}{
		{"known", source, 1},
		{"unknown", "// @filename: /node_modules/effect/package.json\n" + string(unknownPackage) + "\n" + source, 0},
		{"mixed", `// @filename: /node_modules/nested/package.json
{ "name": "nested", "version": "1.0.0", "types": "index.d.ts" }
// @filename: /node_modules/nested/index.d.ts
export { marker } from "effect"
// @filename: /node_modules/nested/node_modules/effect/package.json
{ "name": "effect", "version": "3.19.19", "types": "index.d.ts" }
// @filename: /node_modules/nested/node_modules/effect/index.d.ts
export declare const marker: unique symbol
` + source + `import { marker } from "nested"
export const mixedPackage = marker
`, 0},
	} {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			preview := evaluatePreview(t, bundledeffect.EffectV4, test.source, &rules.EffectSleepZeroToYieldNow)
			if len(preview.Diagnostics) != test.want {
				t.Fatalf("expected %d sleep-zero suggestions, got %d", test.want, len(preview.Diagnostics))
			}
		})
	}
}
