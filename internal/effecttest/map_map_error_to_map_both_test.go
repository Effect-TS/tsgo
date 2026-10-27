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

func TestMapMapErrorToMapBothFixesCompile(t *testing.T) {
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
      "diagnosticSeverity": { "*": "off", "mapMapErrorToMapBoth": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
`
			mainMatches, mainFixes := 25, 24
			// Eager pairs split across an evaluation point have no fix unless the
			// second callback is a function literal.
			orderMatches, orderFixes := 6, 3
			crossMatches, crossFixes := 11, 9
			// Curried pairs keep a fix only when no non-literal expression is reordered
			// and no call they edit has explicit type arguments.
			curriedMatches, curriedFixes := 10, 3
			// Pairs split across calls keep a fix only when no non-literal callback or
			// eager work moves across other code, and no call on the path between the
			// steps has explicit type arguments.
			boundaryMatches, boundaryFixes := 9, 4
			if version == bundledeffect.EffectV3 {
				curriedMatches, curriedFixes = 8, 2
				boundaryMatches, boundaryFixes = 6, 3
				mainMatches, mainFixes = 22, 21
				orderMatches, orderFixes = 2, 2
				crossMatches, crossFixes = 8, 8
			}
			cases := []struct {
				name     string
				source   string
				expected string
				matches  int
				fixes    int
			}{
				{name: "mapMapErrorToMapBoth.ts", matches: mainMatches, fixes: mainFixes},
				{name: "mapMapErrorToMapBoth_preview.ts", matches: 1, fixes: 1},
				// No binding in scope reaches mapBoth, so no fix is offered.
				{name: "mapMapErrorToMapBoth_namedImportsNoTarget.ts", matches: 3, fixes: 0},
				{name: "mapMapErrorToMapBoth_namedImportsTarget.ts", matches: 1, fixes: 1},
				{name: "mapMapErrorToMapBoth_evaluationOrder.ts", matches: orderMatches, fixes: orderFixes},
				{name: "mapMapErrorToMapBoth_crossPipe.ts", matches: crossMatches, fixes: crossFixes},
				{name: "mapMapErrorToMapBoth_curried.ts", matches: curriedMatches, fixes: curriedFixes},
				{name: "mapMapErrorToMapBoth_boundaries.ts", matches: boundaryMatches, fixes: boundaryFixes},
				{
					name: "exactCrossPipe",
					source: `import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const value = source.pipe(Effect.tap(() => Effect.void), Effect.map((n) => n + 1)).pipe(Effect.mapError((e) => e.length), Effect.asVoid)
`,
					expected: `import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const value = source.pipe(Effect.tap(() => Effect.void)).pipe(Effect.mapBoth({
    onSuccess: n => n + 1,
    onFailure: e => e.length
}), Effect.asVoid)
`,
					matches: 1, fixes: 1,
				},
				{
					name: "exactEmptiedPipe",
					source: `import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const value = source.pipe(Effect.mapError((e) => e.length)).pipe(Effect.map((n) => n + 1))
`,
					expected: `import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const value = source.pipe(Effect.mapBoth({
    onFailure: e => e.length,
    onSuccess: n => n + 1
}))
`,
					matches: 1, fixes: 1,
				},
				{
					name: "exactNamedImportTarget",
					source: `import type { Effect } from "effect"
import { map, mapBoth, mapError } from "effect/Effect"
declare const source: Effect.Effect<number, string>
export const value = source.pipe(map((n) => n + 1), mapError((e) => e.length))
`,
					expected: `import type { Effect } from "effect"
import { map, mapBoth, mapError } from "effect/Effect"
declare const source: Effect.Effect<number, string>
export const value = source.pipe(mapBoth({
    onSuccess: n => n + 1,
    onFailure: e => e.length
}))
`,
					matches: 1, fixes: 1,
				},
				{
					name: "exactReverseOrder",
					source: `import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const value = source.pipe(Effect.mapError((e) => e.length), Effect.map((n) => n + 1))
`,
					expected: `import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const value = source.pipe(Effect.mapBoth({
    onFailure: e => e.length,
    onSuccess: n => n + 1
}))
`,
					matches: 1, fixes: 1,
				},
				{
					name: "exactNamespaceAndSurroundingSteps",
					source: `import * as E from "effect/Effect"
declare const source: E.Effect<number, string>
export const value = source.pipe(E.tap(() => E.void), E.map((n) => n + 1), E.mapError((e) => e.length), E.asVoid)
`,
					expected: `import * as E from "effect/Effect"
declare const source: E.Effect<number, string>
export const value = source.pipe(E.tap(() => E.void), E.mapBoth({
    onSuccess: n => n + 1,
    onFailure: e => e.length
}), E.asVoid)
`,
					matches: 1, fixes: 1,
				},
				{
					name: "exactDataFirst",
					source: `import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const value = Effect.map(Effect.mapError(source, (e) => e.length), (n) => n + 1)
`,
					expected: `import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const value = Effect.mapBoth(source, {
    onFailure: e => e.length,
    onSuccess: n => n + 1
})
`,
					matches: 1, fixes: 1,
				},
			}
			code := tsdiag.Effect_0_Effect_1_transform_the_success_and_failure_channels_one_step_at_a_time_Effect_2_expresses_both_transformations_in_a_single_step_effect_mapMapErrorToMapBoth.Code()
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
						source = strings.ReplaceAll(string(fixture), "mapMapErrorToMapBoth:warning", "mapMapErrorToMapBoth:suggestion")
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
							if title != "Replace with Effect.mapBoth" && title != "Replace with Effect.mapBothEager" {
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
