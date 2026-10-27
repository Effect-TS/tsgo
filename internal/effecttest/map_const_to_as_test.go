package effecttest_test

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/effecttest"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/fourslash"
	"github.com/microsoft/TypeScript/tsc/shim/ls/lsconv"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
)

func TestMapConstToAsFixesCompile(t *testing.T) {
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
      "diagnosticSeverity": { "*": "off", "mapConstToAs": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
`
			cases := []struct {
				name     string
				source   string
				expected string
				matches  int
			}{
				{name: "mapConstToAs.ts", matches: 23},
				{
					// A return annotation pins the success type; Effect.as would widen it.
					name: "annotatedReturn",
					source: `import { Effect } from "effect"
declare const source: Effect.Effect<number>
const result = source.pipe(Effect.map((): "saved" => "saved"))
export const checked: Effect.Effect<"saved"> = result
`,
					matches: 0,
				},
				{
					name: "dataFirst",
					source: `import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = Effect.map(source, () => "saved")
`,
					expected: `import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = Effect.as(source, "saved")
`,
					matches: 1,
				},
				{
					name: "dataLastApplied",
					source: `import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = Effect.map(() => -1)(source)
`,
					expected: `import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = Effect.as(-1)(source)
`,
					matches: 1,
				},
				{
					name: "aliasFallback",
					source: `import * as Fx from "effect/Effect"
declare const source: Fx.Effect<number>
const mapAlias = Fx.map
export const value = source.pipe(mapAlias(() => "saved"))
`,
					expected: `import * as Fx from "effect/Effect"
declare const source: Fx.Effect<number>
const mapAlias = Fx.map
export const value = source.pipe(Fx.as("saved"))
`,
					matches: 1,
				},
				{
					name: "blockReturnInPipe",
					source: `import { Effect } from "effect"
enum Phase { Done = "done" }
declare const source: Effect.Effect<number>
export const value = source.pipe(
  Effect.tap(() => Effect.void),
  Effect.map((_) => {
    return Phase.Done
  }),
  Effect.map((phase) => phase.length)
)
`,
					expected: `import { Effect } from "effect"
enum Phase { Done = "done" }
declare const source: Effect.Effect<number>
export const value = source.pipe(
  Effect.tap(() => Effect.void),
  Effect.as(Phase.Done),
  Effect.map((phase) => phase.length)
)
`,
					matches: 1,
				},
			}
			code := tsdiag.Effect_as_expresses_replacing_the_success_value_with_a_constant_directly_effect_mapConstToAs.Code()
			uri := lsconv.FileNameToDocumentURI(tspath.RootedFilePath("/repro.ts"))
			countMatches := func(f *fourslash.FourslashTest) int {
				count := 0
				for _, diagnostic := range f.GetQuickFixesForDiagnostics(t, uri) {
					if diagnostic.Diagnostic.Code != nil && diagnostic.Diagnostic.Code.Integer != nil && *diagnostic.Diagnostic.Code.Integer == code {
						count++
					}
				}
				return count
			}
			for _, test := range cases {
				t.Run(test.name, func(t *testing.T) {
					source := test.source
					if source == "" {
						fixture, err := os.ReadFile(filepath.Join(effecttest.TestDataPath(), "tests", string(version), test.name))
						if err != nil {
							t.Fatal(err)
						}
						source = string(fixture)
					}
					source = "// @" + string(version) + "\n" + source
					f, done := fourslash.NewFourslash(t, nil, prefix+source)
					defer done()
					f.GoToFile(t, "/repro.ts")
					f.VerifyNonSuggestionDiagnostics(t, nil)
					matches, fixes := 0, 0
					for _, diagnostic := range f.GetQuickFixesForDiagnostics(t, uri) {
						if diagnostic.Diagnostic.Code == nil || diagnostic.Diagnostic.Code.Integer == nil || *diagnostic.Diagnostic.Code.Integer != code {
							continue
						}
						matches++
						for index, title := range diagnostic.FixTitles {
							if title != "Replace with Effect.as" {
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
								if remaining := countMatches(converted); remaining != test.matches-1 {
									t.Fatalf("expected %d diagnostics after replacement, got %d", test.matches-1, remaining)
								}
							}()
							fixes++
						}
					}
					if matches != test.matches || fixes != test.matches {
						t.Fatalf("got %d diagnostics and %d compiled fixes, want %d of each", matches, fixes, test.matches)
					}
				})
			}
		})
	}
}
