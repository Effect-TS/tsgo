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

func TestFlatMapIdentityToFlattenFixesCompile(t *testing.T) {
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
      "diagnosticSeverity": { "*": "off", "flatMapIdentityToFlatten": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
`
			mainMatches := 16
			if version == bundledeffect.EffectV3 {
				mainMatches = 17
			}
			cases := []struct {
				name     string
				source   string
				expected string
				matches  int
				fixes    int
			}{
				{name: "flatMapIdentityToFlatten.ts", matches: mainMatches, fixes: mainMatches},
				{
					name: "exactPipeable",
					source: `import * as Fx from "effect/Effect"
declare const source: Fx.Effect<Fx.Effect<number>>
export const value = source.pipe(Fx.flatMap((inner) => /* unwrap */ inner))
`,
					expected: `import * as Fx from "effect/Effect"
declare const source: Fx.Effect<Fx.Effect<number>>
export const value = source.pipe(Fx.flatten)
`,
					matches: 1, fixes: 1,
				},
				{
					name: "exactDataFirst",
					source: `import { Effect, identity } from "effect"
declare const source: Effect.Effect<Effect.Effect<number>>
export const value = Effect.flatMap(source, identity)
`,
					expected: `import { Effect, identity } from "effect"
declare const source: Effect.Effect<Effect.Effect<number>>
export const value = Effect.flatten(source)
`,
					matches: 1, fixes: 1,
				},
				{
					name: "dataLastIgnored",
					source: `import { Effect } from "effect"
declare const source: Effect.Effect<Effect.Effect<number>>
export const value = Effect.flatMap((inner: Effect.Effect<number>) => inner)(source)
`,
					matches: 0, fixes: 0,
				},
				{
					name: "typedAliasWideningError",
					source: `import { Effect, identity } from "effect"
declare const source: Effect.Effect<Effect.Effect<string>>
const unwrap: (x: Effect.Effect<string>) => Effect.Effect<string, Error> = identity
export const value = source.pipe(Effect.flatMap(unwrap))
export const handled = value.pipe(Effect.mapError((error) => error.message))
`,
					matches: 0, fixes: 0,
				},
				{
					name: "typedAliasAnySuccess",
					source: `import { Effect, identity } from "effect"
declare const source: Effect.Effect<Effect.Effect<string>>
const unwrap: (x: Effect.Effect<string>) => Effect.Effect<any> = identity
export const value = source.pipe(Effect.flatMap(unwrap))
export const mapped = value.pipe(Effect.map((value) => value.toFixed()))
`,
					matches: 0, fixes: 0,
				},
				{
					name: "typedAliasMatchingIgnored",
					source: `import { Effect, identity } from "effect"
declare const source: Effect.Effect<Effect.Effect<string>>
const unwrap: (x: Effect.Effect<string>) => Effect.Effect<string> = identity
export const value = source.pipe(Effect.flatMap(unwrap))
`,
					matches: 0, fixes: 0,
				},
				{
					name: "namedFlatMapHasNoFix",
					source: `import type { Effect } from "effect"
import { flatMap } from "effect/Effect"
declare const source: Effect.Effect<Effect.Effect<number>>
export const value = source.pipe(flatMap((inner) => inner))
`,
					matches: 1, fixes: 0,
				},
			}
			if version == bundledeffect.EffectV4 {
				cases = append(cases, struct {
					name     string
					source   string
					expected string
					matches  int
					fixes    int
				}{name: "flatMapIdentityToFlatten_preview.ts", matches: 1, fixes: 1})
			}
			code := tsdiag.Effect_flatten_expresses_this_unwrapping_more_directly_than_Effect_flatMap_with_an_identity_callback_effect_flatMapIdentityToFlatten.Code()
			uri := lsconv.FileNameToDocumentURI(tspath.RootedFilePath("/repro.ts"))
			for _, test := range cases {
				t.Run(test.name, func(t *testing.T) {
					source := test.source
					if source == "" {
						fixture, err := os.ReadFile(filepath.Join(effecttest.TestDataPath(), "tests", string(version), test.name))
						if err != nil {
							t.Fatal(err)
						}
						// The fixtures raise the rule to a warning; keep it a suggestion so that
						// only real compile errors fail VerifyNonSuggestionDiagnostics.
						source = strings.Replace(string(fixture), "flatMapIdentityToFlatten:warning", "flatMapIdentityToFlatten:suggestion", 1)
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
							if title != "Replace with Effect.flatten" {
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
