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

func TestFailCauseToConstructorFixesCompile(t *testing.T) {
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
      "diagnosticSeverity": { "*": "off", "failCauseToConstructor": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
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
					name: "pipePrefix",
					source: `import { Cause, Effect, pipe } from "effect"
declare const defect: unknown
export const value = pipe(Cause.die(defect), Effect.failCause, Effect.asVoid)
`,
					expected: `import { Cause, Effect, pipe } from "effect"
declare const defect: unknown
export const value = pipe(Effect.die(defect), Effect.asVoid)
`,
					matches: 1,
					fixes:   1,
				},
				{
					name: "namedImportsLazy",
					source: `import { Effect } from "effect"
import { fail as causeFail } from "effect/Cause"
import { failCauseSync } from "effect/Effect"
declare const error: Error
export const value = failCauseSync(() => causeFail(error))
`,
					expected: `import { Effect } from "effect"
import { fail as causeFail } from "effect/Cause"
import { failCauseSync } from "effect/Effect"
declare const error: Error
export const value = Effect.failSync(() => error)
`,
					matches: 1,
					fixes:   1,
				},
				{
					name: "lazyBlock",
					source: `import { Cause, Effect } from "effect"
declare const error: Error
export const value = Effect.failCauseSync(() => {
  return Cause.fail(error)
})
`,
					expected: `import { Cause, Effect } from "effect"
declare const error: Error
export const value = Effect.failSync(() => {
  return error
})
`,
					matches: 1,
					fixes:   1,
				},
				{
					// A bare object literal as a concise arrow body would parse as a block.
					name: "lazyObjectLiteral",
					source: `import { Cause, Effect } from "effect"
export const value = Effect.failCauseSync(() => Cause.fail({ _tag: "E" }))
`,
					expected: `import { Cause, Effect } from "effect"
export const value = Effect.failSync(() => ({ _tag: "E" }))
`,
					matches: 1,
					fixes:   1,
				},
				{
					// Cause.fail widens the literal union to string; a failSync thunk would
					// keep "A" | "B", so the fix is withheld.
					name: "lazyLiteralUnion",
					source: `import { Cause, Effect } from "effect"
declare const condition: boolean
export const value = Effect.failCauseSync(() => Cause.fail(condition ? "A" : "B"))
`,
					matches: 1,
				},
				{
					// Without an Effect module binding the fallback receiver would be unbound.
					name: "namedImportsOnly",
					source: `import { die as causeDie, fail as causeFail } from "effect/Cause"
import { failCause, failCauseSync } from "effect/Effect"
declare const error: Error
export const eager = failCause(causeDie(error))
export const lazy = failCauseSync(() => causeFail(error))
`,
					matches: 2,
				},
				{
					// A type-only Effect binding is not a value; Effect.fail would be TS1361.
					name: "typeOnlyEffect",
					source: `import type { Effect } from "effect"
import { fail as causeFail } from "effect/Cause"
import { failCause, failCauseSync } from "effect/Effect"
declare const error: Error
export const eager: Effect.Effect<never, Error> = failCause(causeFail(error))
export const lazy: Effect.Effect<never, Error> = failCauseSync(() => causeFail(error))
`,
					matches: 2,
				},
				{
					// The lazy form as a pipe subject is reported without a fix.
					name: "lazyFunctionPipe",
					source: `import { Cause, Effect, pipe } from "effect"
declare const error: Error
export const value = pipe(() => Cause.fail(error), Effect.failCauseSync)
`,
					matches: 1,
				},
			}
			if version == bundledeffect.EffectV4 {
				cases = append(cases,
					fixCase{name: "failCauseToConstructor.ts", matches: 24, fixes: 22},
					fixCase{name: "failCauseToConstructor_preview.ts", matches: 1, fixes: 1},
				)
			} else {
				cases = append(cases,
					fixCase{name: "failCauseToConstructor.ts", matches: 24, fixes: 21},
					fixCase{
						name: "lazyDie",
						source: `import * as Cause from "effect/Cause"
import * as Effect from "effect/Effect"
export const value = Effect.failCauseSync(() => Cause.die(new Error("late")))
`,
						expected: `import * as Cause from "effect/Cause"
import * as Effect from "effect/Effect"
export const value = Effect.dieSync(() => new Error("late"))
`,
						matches: 1,
						fixes:   1,
					},
				)
			}
			cases = append(cases,
				fixCase{name: "failCauseToConstructor_namedImports.ts", matches: 3},
				fixCase{name: "failCauseToConstructor_aliasedEffect.ts", matches: 2, fixes: 2},
			)
			code := tsdiag.Effect_0_expresses_this_failure_directly_instead_of_wrapping_Cause_2_in_Effect_1_effect_failCauseToConstructor.Code()
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
						source = strings.Replace(string(fixture), "failCauseToConstructor:warning", "failCauseToConstructor:suggestion", 1)
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
