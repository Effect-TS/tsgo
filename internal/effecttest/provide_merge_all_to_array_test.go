package effecttest_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/effecttest"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/fourslash"
	"github.com/microsoft/TypeScript/tsc/shim/ls/lsconv"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
)

func TestProvideMergeAllToArrayFixesCompile(t *testing.T) {
	t.Parallel()
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			const prefix = `// @filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "skipLibCheck": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [{
      "name": "@effect/language-service",
      "diagnosticSeverity": { "*": "off", "provideMergeAllToArray": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
`
			// The v3 fixture has no options cases: v3 Effect.provide takes no options.
			mainMatches := 18
			if version == bundledeffect.EffectV3 {
				mainMatches = 16
			}
			cases := []struct {
				name     string
				source   string
				expected string
				matches  int
				fixes    int
			}{
				{name: "provideMergeAllToArray.ts", matches: mainMatches, fixes: mainMatches},
				{name: "provideMergeAllToArray_preview.ts", matches: 1, fixes: 1},
				{
					// A non-ASCII identifier before the call shifts byte offsets away
					// from LSP positions.
					name: "exactNonAsciiPrefix",
					source: `import { Effect, Layer } from "effect"
declare const ALayer: Layer.Layer<"A">
declare const BLayer: Layer.Layer<"B">
declare const program: Effect.Effect<void, never, "A" | "B">
const café = "ü"
export const value = [café, program.pipe(Effect.provide(/* app */ Layer.mergeAll(ALayer, BLayer /* last */)))]
`,
					expected: `import { Effect, Layer } from "effect"
declare const ALayer: Layer.Layer<"A">
declare const BLayer: Layer.Layer<"B">
declare const program: Effect.Effect<void, never, "A" | "B">
const café = "ü"
export const value = [café, program.pipe(Effect.provide(/* app */ [ALayer, BLayer /* last */]))]
`,
					matches: 1, fixes: 1,
				},
			}
			code := tsdiag.Effect_provide_accepts_an_array_of_layers_and_merges_them_itself_pass_the_layers_as_an_array_instead_of_wrapping_them_in_Layer_mergeAll_effect_provideMergeAllToArray.Code()
			uri := lsconv.FileNameToDocumentURI(tspath.RootedFilePath("/repro.ts"))
			for _, test := range cases {
				t.Run(test.name, func(t *testing.T) {
					source := test.source
					if source == "" {
						fixture, err := os.ReadFile(filepath.Join(effecttest.TestDataPath(), "tests", string(version), test.name))
						if err != nil {
							t.Fatal(err)
						}
						// Keep the rule at suggestion severity so only compile errors are non-suggestions.
						source = strings.ReplaceAll(string(fixture), "provideMergeAllToArray:warning", "provideMergeAllToArray:suggestion")
					}
					source = "// @" + string(version) + "\n" + source
					f, done := fourslash.NewFourslash(t, nil, prefix+source)
					defer done()
					f.GoToFile(t, "/repro.ts")
					f.VerifyNonSuggestionDiagnostics(t, nil)
					inventory := f.GetQuickFixesForDiagnostics(t, uri)
					matches, checked := 0, 0
					for _, diagnostic := range inventory {
						if diagnostic.Diagnostic.Code == nil || diagnostic.Diagnostic.Code.Integer == nil || *diagnostic.Diagnostic.Code.Integer != code {
							continue
						}
						matches++
						for index, title := range diagnostic.FixTitles {
							if title != "Pass the layers as an array to Effect.provide" {
								continue
							}
							result := f.ApplyQuickFix(t, uri, diagnostic.Diagnostic, index)
							if len(result.Changes) != 1 {
								t.Fatalf("expected one changed file, got %d", len(result.Changes))
							}
							after := result.Changes[0].After
							if test.expected != "" && after != "// @"+string(version)+"\n"+test.expected {
								t.Fatalf("replacement = %q, want %q", after, "// @"+string(version)+"\n"+test.expected)
							}
							func() {
								converted, closeConverted := fourslash.NewFourslash(t, nil, prefix+after)
								defer closeConverted()
								converted.GoToFile(t, "/repro.ts")
								converted.VerifyNonSuggestionDiagnostics(t, nil)
								remaining := 0
								for _, diagnostic := range converted.GetQuickFixesForDiagnostics(t, uri) {
									if diagnostic.Diagnostic.Code != nil && diagnostic.Diagnostic.Code.Integer != nil && *diagnostic.Diagnostic.Code.Integer == code {
										remaining++
									}
								}
								if remaining != test.matches-1 {
									t.Fatalf("expected %d diagnostics after replacement, got %d", test.matches-1, remaining)
								}
							}()
							checked++
						}
					}
					if matches != test.matches || checked != test.fixes {
						t.Fatalf("got %d diagnostics and %d compiled fixes, want %d and %d", matches, checked, test.matches, test.fixes)
					}
				})
			}
		})
	}
}
