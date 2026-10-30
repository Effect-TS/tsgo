package effecttest_test

import (
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/fourslash"
	"github.com/microsoft/TypeScript/tsc/shim/ls/lsutil"

	_ "github.com/effect-ts/tsgo/etslshooks"
	_ "github.com/effect-ts/tsgo/etstesthooks"
)

func TestEffectInlayHints(t *testing.T) {
	t.Parallel()

	for _, tc := range []struct {
		name          string
		pluginOptions string
		source        string
	}{
		{
			name: "effectInlayHintsGenSuppression",
			pluginOptions: `,
        "inlays": true`,
			source: `import { Effect } from "effect"

export function standardShouldAppear() {
  return 42
}

export const sample = Effect.gen(function*() {
  const n = Math.random()
  if (n < 0.5) {
    return yield* Effect.fail("Error")
  }
  return n
})`,
		},
		{
			name: "effectInlayHintsFnSuppression",
			pluginOptions: `,
        "inlays": true`,
			source: `import { Effect } from "effect"

export function standardShouldAppear() {
  return 42
}

export const sampleFn = Effect.fn("sampleFn")(function*(
  _arg1: number,
  _arg2: string
) {
  const n = Math.random()
  if (n < 0.5) {
    return yield* Effect.fail("Error")
  }
  return n
})`,
		},
		{
			name: "effectInlayHintsFnUntracedSuppression",
			pluginOptions: `,
        "inlays": true`,
			source: `import { Effect } from "effect"

export function standardShouldAppear() {
  return 42
}

export const sampleFnUntraced = Effect.fnUntraced(function*(_: boolean) {
  const n = Math.random()
  if (n < 0.5) {
    return yield* Effect.fail("Error")
  }
  return n
})`,
		},
		{
			name: "effectInlayHintsDisabledPassthrough",
			source: `import { Effect } from "effect"

export function standardShouldAppear() {
  return 42
}

export const sample = Effect.gen(function*() {
  const n = Math.random()
  if (n < 0.5) {
    return yield* Effect.fail("Error")
  }
  return n
})`,
		},
	} {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			content := pluginTestContent(tc.pluginOptions, tc.source)
			f, done := fourslash.NewFourslash(t, nil, content)
			defer done()

			f.GoToFile(t, "/test.ts")
			verifyLocalBaselineInlayHints(t, f, content, "/test.ts", lsutil.NewInlayHintUserPreferences(lsutil.InlayHintsPreferences{
				IncludeInlayFunctionLikeReturnTypeHints: core.TSTrue,
			}))
		})
	}
}
