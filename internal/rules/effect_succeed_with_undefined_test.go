package rules_test

import (
	"encoding/json"
	"testing"
	"testing/fstest"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/rules"
)

// unknownEffectVersionPrefix returns a preview prefix that replaces the bundled v4
// effect package.json with one that has no version, so the Effect version is unknown.
func unknownEffectVersionPrefix(t *testing.T) string {
	t.Helper()
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
	return "// @filename: /node_modules/effect/package.json\n" + string(unknownPackage) + "\n"
}

func TestEffectSucceedWithUndefinedCallShapes(t *testing.T) {
	t.Parallel()
	const header = "// @filename: preview.ts\nimport { Effect, pipe } from \"effect\"\n"
	for _, test := range []struct {
		name   string
		source string
		want   int
	}{
		{"plain", "export const a = Effect.succeed(undefined)\n", 1},
		{"data-first argument", "export const a = Effect.andThen(Effect.void, Effect.succeed(undefined))\n", 1},
		{"pipe subject", "export const a = pipe(Effect.succeed(undefined), Effect.as(1))\n", 1},
		{"Effect.fn body", "export const a = Effect.fn(function*() { return yield* Effect.succeed(undefined) })\n", 1},
		{"Effect.fnUntraced body", "export const a = Effect.fnUntraced(function*() { return yield* Effect.succeed(undefined) })\n", 1},
		{"undefined-typed shadowing parameter", "export const a = (undefined: undefined) => Effect.succeed(undefined)\n", 0},
		{"point-free pipe", "export const a = pipe(undefined, Effect.succeed)\n", 1},
		{"point-free pipe with steps", "export const a = pipe((void 0), Effect.succeed, Effect.as(1))\n", 1},
		{"point-free pipe as a data-first argument", "export const a = Effect.andThen(Effect.void, pipe(undefined, Effect.succeed))\n", 1},
		{"void 0", "export const a = Effect.succeed(void 0)\n", 1},
		{"explicit void type argument", "export const a = Effect.succeed<void>(undefined)\n", 0},
	} {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			preview := evaluatePreview(t, bundledeffect.EffectV4, header+test.source, &rules.EffectSucceedWithUndefined)
			if len(preview.Diagnostics) != test.want {
				t.Fatalf("expected %d effectSucceedWithUndefined diagnostics, got %d", test.want, len(preview.Diagnostics))
			}
		})
	}
}

func TestEffectSucceedWithUndefinedVersionGate(t *testing.T) {
	t.Parallel()
	const source = `// @filename: preview.ts
import { Effect } from "effect"
export const a = Effect.succeed(undefined)
export const b = Effect.succeed(void 0)
export const c = Effect.succeed<void>(undefined)
export const d = Effect.succeed<number | undefined>(undefined)
`
	unknown := unknownEffectVersionPrefix(t)
	for _, test := range []struct {
		name          string
		version       bundledeffect.EffectVersion
		prefix        string
		wantUndefined int
		wantVoid      int
	}{
		{"v4", bundledeffect.EffectV4, "", 2, 1},
		{"v3", bundledeffect.EffectV3, "", 0, 3},
		{"unknown", bundledeffect.EffectV4, unknown, 0, 3},
	} {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			undefinedPreview := evaluatePreview(t, test.version, test.prefix+source, &rules.EffectSucceedWithUndefined)
			if len(undefinedPreview.Diagnostics) != test.wantUndefined {
				t.Fatalf("expected %d effectSucceedWithUndefined diagnostics, got %d", test.wantUndefined, len(undefinedPreview.Diagnostics))
			}
			voidPreview := evaluatePreview(t, test.version, test.prefix+source, &rules.EffectSucceedWithVoid)
			if len(voidPreview.Diagnostics) != test.wantVoid {
				t.Fatalf("expected %d effectSucceedWithVoid diagnostics, got %d", test.wantVoid, len(voidPreview.Diagnostics))
			}
		})
	}
}
