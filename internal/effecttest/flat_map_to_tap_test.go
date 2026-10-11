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

func TestFlatMapToTapFixesCompile(t *testing.T) {
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
      "diagnosticSeverity": { "*": "off", "flatMapToTap": "suggestion", "promiseInEffectSuccess": "off" }
    }]
  }
}
// @filename: /repro.ts
`
			mainMatches := 23
			if version == bundledeffect.EffectV3 {
				mainMatches = 22
			}
			cases := []struct {
				name     string
				source   string
				expected string
				matches  int
				fixes    int
			}{
				{name: "flatMapToTap.ts", matches: mainMatches, fixes: mainMatches},
				{name: "flatMapToTap_aliases.ts", matches: 6, fixes: 6},
				{name: "flatMapToTap_preview.ts", matches: 1, fixes: 1},
				{name: "flatMapToTap_noTarget.ts", matches: 1, fixes: 0},
				{
					name: "exactNamedTap",
					source: `import type { Effect } from "effect"
import { as as restore, flatMap as bind, tap as observe } from "effect/Effect"
declare const source: Effect.Effect<number>
declare const audit: Effect.Effect<void>
const aTap: unknown = observe
export const value = source.pipe(bind((n) => restore(audit, n)))
void aTap
`,
					expected: `import type { Effect } from "effect"
import { as as restore, flatMap as bind, tap as observe } from "effect/Effect"
declare const source: Effect.Effect<number>
declare const audit: Effect.Effect<void>
const aTap: unknown = observe
export const value = source.pipe(observe((n) => audit))
void aTap
`,
					matches: 1, fixes: 1,
				},
				{
					name: "exactDirect",
					source: `import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = Effect.flatMap(source, (n) => Effect.as(Effect.log(n), n))
`,
					expected: `import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = Effect.tap(source, (n) => Effect.log(n))
`,
					matches: 1, fixes: 1,
				},
				{
					name: "exactPipeComment",
					source: `import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.flatMap((n) => Effect.log(n).pipe(
  Effect.map(() => 1), // Keep this explanation of the prior step.
  Effect.as(n)
)))
`,
					expected: `import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.tap((n) => Effect.log(n).pipe(
  Effect.map(() => 1) // Keep this explanation of the prior step.

)))
`,
					matches: 1, fixes: 1,
				},
				{
					name: "exactPipe",
					source: `import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.andThen((n) => {
  // Keep the callback's logging.
  console.log(n)
  return Effect.log(/* keep this argument */ n).pipe(Effect.map(() => 1), Effect.as(n))
}))
`,
					expected: `import { Effect } from "effect"
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.tap((n) => {
  // Keep the callback's logging.
  console.log(n)
  return Effect.log(/* keep this argument */ n).pipe(Effect.map(() => 1))
}))
`,
					matches: 1, fixes: 1,
				},
			}
			code := tsdiag.Effect_tap_expresses_this_observation_more_directly_than_Effect_0_with_a_manual_re_yield_effect_flatMapToTap.Code()
			uri := lsconv.FileNameToDocumentURI(tspath.RootedFilePath("/repro.ts"))
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
					inventory := f.GetQuickFixesForDiagnostics(t, uri)
					matches, checked := 0, 0
					for _, diagnostic := range inventory {
						if diagnostic.Diagnostic.Code == nil || diagnostic.Diagnostic.Code.Integer == nil || *diagnostic.Diagnostic.Code.Integer != code {
							continue
						}
						matches++
						before := checked
						for index, title := range diagnostic.FixTitles {
							if title != "Replace with Effect.tap" {
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
							if test.name == "flatMapToTap.ts" && strings.Contains(result.Changes[0].Before, "original user") && !strings.Contains(after, "original user") {
								t.Fatal("replacement removed an inner comment")
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
						if before == checked {
							t.Logf("no fix at line %d", diagnostic.Diagnostic.Range.Start.Line+1)
						}
					}
					if matches != test.matches || checked != test.fixes {
						t.Fatalf("got %d diagnostics and %d compiled fixes, want %d and %d", matches, checked, test.matches, test.fixes)
					}
					t.Logf("compiled %d tap replacements", checked)
				})
			}
		})
	}
}
