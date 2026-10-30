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

func TestSuspendFailToFailSyncFixesCompile(t *testing.T) {
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
      "diagnosticSeverity": { "*": "off", "suspendFailToFailSync": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
`
			effectImport := `import { Effect } from "effect"`
			if version == bundledeffect.EffectV3 {
				effectImport = `import * as Effect from "effect/Effect"`
			}
			// exact fails to compile unless the value keeps exactly the given type,
			// so a fix that changes the inferred error type does not compile.
			const exact = `type Exact<A, B> = (<T>() => T extends A ? 1 : 2) extends (<T>() => T extends B ? 1 : 2) ? true : false
`
			type fixCase struct {
				name     string
				source   string
				expected string
				matches  int
				fixes    int
			}
			cases := []fixCase{
				{
					// A single literal widens to string in both forms.
					name: "singleLiteral",
					source: effectImport + `
` + exact + `export const value = Effect.suspend(() => Effect.fail("not found"))
export const check: Exact<typeof value, Effect.Effect<never, string>> = true
`,
					expected: effectImport + `
` + exact + `export const value = Effect.failSync(() => "not found")
export const check: Exact<typeof value, Effect.Effect<never, string>> = true
`,
					matches: 1,
					fixes:   1,
				},
				{
					name: "innerTypeArgument",
					source: effectImport + `
` + exact + `declare const error: Error
export const value = Effect.suspend(() => Effect.fail<Error | string>(error))
export const check: Exact<typeof value, Effect.Effect<never, Error | string>> = true
`,
					expected: effectImport + `
` + exact + `declare const error: Error
export const value = Effect.failSync<Error | string>(() => error)
export const check: Exact<typeof value, Effect.Effect<never, Error | string>> = true
`,
					matches: 1,
					fixes:   1,
				},
				{
					// A bare object literal as a concise arrow body would parse as a block.
					name: "objectLiteral",
					source: effectImport + `
` + exact + `export const value = Effect.suspend(() => Effect.fail({ _tag: "Oops" }))
export const check: Exact<typeof value, Effect.Effect<never, { _tag: string }>> = true
`,
					expected: effectImport + `
` + exact + `export const value = Effect.failSync(() => ({ _tag: "Oops" }))
export const check: Exact<typeof value, Effect.Effect<never, { _tag: string }>> = true
`,
					matches: 1,
					fixes:   1,
				},
				{
					name: "unionBinding",
					source: effectImport + `
` + exact + `declare const tag: "A" | "B"
export const value = Effect.suspend(() => Effect.fail(tag))
export const check: Exact<typeof value, Effect.Effect<never, "A" | "B">> = true
`,
					expected: effectImport + `
` + exact + `declare const tag: "A" | "B"
export const value = Effect.failSync(() => tag)
export const check: Exact<typeof value, Effect.Effect<never, "A" | "B">> = true
`,
					matches: 1,
					fixes:   1,
				},
				{
					// The suspend and the fail as pipe steps keep the error type.
					name: "pipeSteps",
					source: effectImport + `
import { pipe } from "effect/Function"
` + exact + `declare const tag: "A" | "B"
export const value = pipe(() => pipe(tag, Effect.fail), Effect.suspend)
export const check: Exact<typeof value, Effect.Effect<never, "A" | "B">> = true
`,
					expected: effectImport + `
import { pipe } from "effect/Function"
` + exact + `declare const tag: "A" | "B"
export const value = pipe(() => tag, Effect.failSync)
export const check: Exact<typeof value, Effect.Effect<never, "A" | "B">> = true
`,
					matches: 1,
					fixes:   1,
				},
				{
					// Effect.fail widens the literal union to string; a failSync thunk would
					// keep "A" | "B", so the fix is withheld.
					name: "literalUnion",
					source: effectImport + `
declare const condition: boolean
export const value = Effect.suspend(() => Effect.fail(condition ? "A" : "B"))
`,
					matches: 1,
				},
				{
					name: "namedImports",
					source: effectImport + `
import { fail, suspend } from "effect/Effect"
declare const error: Error
export const value = suspend(() => fail(error))
`,
					expected: effectImport + `
import { fail, suspend } from "effect/Effect"
declare const error: Error
export const value = Effect.failSync(() => error)
`,
					matches: 1,
					fixes:   1,
				},
				{
					// Without an Effect module binding the fallback receiver would be unbound.
					name: "namedImportsOnly",
					source: `import { fail, suspend } from "effect/Effect"
declare const error: Error
export const value = suspend(() => fail(error))
`,
					matches: 1,
				},
				{
					// The parameter shadows the file's Effect binding where the fix would write Effect.failSync.
					name: "shadowedEffect",
					source: effectImport + `
import { fail, suspend } from "effect/Effect"
export const value = (Effect: { readonly name: string }) => suspend(() => fail(Effect.name))
`,
					matches: 1,
				},
			}
			if version == bundledeffect.EffectV4 {
				cases = append(cases,
					fixCase{name: "suspendFailToFailSync.ts", matches: 20, fixes: 19},
					fixCase{name: "suspendFailToFailSync_preview.ts", matches: 1, fixes: 1},
					fixCase{name: "suspendFailToFailSync_shapes.ts", matches: 15, fixes: 14},
				)
			} else {
				cases = append(cases,
					fixCase{name: "suspendFailToFailSync.ts", matches: 25, fixes: 24},
					fixCase{name: "suspendFailToFailSync_shapes.ts", matches: 18, fixes: 17},
					fixCase{
						name: "dieSync",
						source: effectImport + `
export const value = Effect.suspend(() => Effect.die(new Error("late")))
`,
						expected: effectImport + `
export const value = Effect.dieSync(() => new Error("late"))
`,
						matches: 1,
						fixes:   1,
					},
				)
			}
			code := tsdiag.Effect_0_builds_this_1_lazily_directly_instead_of_wrapping_Effect_2_in_Effect_suspend_effect_suspendFailToFailSync.Code()
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
						// The fixture enables the rule as a warning; keep it a suggestion so
						// VerifyNonSuggestionDiagnostics only sees compile errors.
						source = strings.Replace(string(fixture), "suspendFailToFailSync:warning", "suspendFailToFailSync:suggestion", 1)
					}
					if !strings.HasPrefix(source, "// @"+string(version)+"\n") {
						source = "// @" + string(version) + "\n" + source
					}
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
							if !strings.HasPrefix(title, "Replace with Effect.") {
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
					if matches != test.matches || fixes != test.fixes {
						t.Fatalf("got %d diagnostics and %d compiled fixes, want %d and %d", matches, fixes, test.matches, test.fixes)
					}
				})
			}
		})
	}
}
