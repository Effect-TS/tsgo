package rules_test

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/rules"
)

func TestEffectSleepZeroToYieldNowUnknownVersion(t *testing.T) {
	t.Parallel()
	source := unknownEffectVersionPrefix(t) + `// @filename: preview.ts
import { Effect } from "effect"
export const sleep = Effect.sleep(0)
`
	preview := evaluatePreviewWithoutHooks(t, bundledeffect.EffectV4, source, &rules.EffectSleepZeroToYieldNow)
	if len(preview.Diagnostics) != 0 {
		t.Fatalf("expected no sleep-zero suggestions, got %d", len(preview.Diagnostics))
	}
}
