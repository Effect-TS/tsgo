package rules_test

import (
	"encoding/json"
	"testing"
	"testing/fstest"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/rules"
)

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

func TestEffectSucceedWithUndefinedUnknownVersion(t *testing.T) {
	t.Parallel()
	source := unknownEffectVersionPrefix(t) + `// @filename: preview.ts
import { Effect } from "effect"
export const a = Effect.succeed(undefined)
export const b = Effect.succeed(void 0)
export const c = Effect.succeed<void>(undefined)
export const d = Effect.succeed<number | undefined>(undefined)
`
	if got := len(evaluatePreviewWithoutHooks(t, bundledeffect.EffectV4, source, &rules.EffectSucceedWithUndefined).Diagnostics); got != 0 {
		t.Fatalf("expected no effectSucceedWithUndefined diagnostics, got %d", got)
	}
	if got := len(evaluatePreviewWithoutHooks(t, bundledeffect.EffectV4, source, &rules.EffectSucceedWithVoid).Diagnostics); got != 3 {
		t.Fatalf("expected 3 effectSucceedWithVoid diagnostics, got %d", got)
	}
}
