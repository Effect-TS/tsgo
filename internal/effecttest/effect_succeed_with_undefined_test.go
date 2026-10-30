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

func TestEffectSucceedWithUndefinedFixesCompile(t *testing.T) {
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
      "diagnosticSeverity": { "*": "off", "effectSucceedWithUndefined": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
`
	sources := map[string]string{
		"callShapes": `import { Effect, pipe } from "effect"
export const dataFirst = Effect.andThen(Effect.void, Effect.succeed(undefined))
export const pipeSubject = pipe(Effect.succeed(undefined), Effect.as(1))
export const fn = Effect.fn(function*() { return yield* Effect.succeed(undefined) })
export const fnUntraced = Effect.fnUntraced(function*() { return yield* Effect.succeed(undefined) })
`,
		"namedSucceedOnly": `import { succeed } from "effect/Effect"
// NO replacement quickfix: no binding reaches Effect.undefined
export const result = succeed(undefined)
`,
		"namedUndefinedImport": `import { succeed, undefined as undefinedEffect } from "effect/Effect"
export const result = succeed(undefined)
export const used = undefinedEffect
`,
	}
	for _, name := range []string{"effectSucceedWithUndefined.ts", "effectSucceedWithUndefined_aliases.ts", "effectSucceedWithUndefined_preview.ts", "effectSucceedWithUndefined_namedImport.ts", "effectSucceedWithUndefined_scopedEffect.ts"} {
		fixture, err := os.ReadFile(filepath.Join(effecttest.TestDataPath(), "tests", string(bundledeffect.EffectV4), name))
		if err != nil {
			t.Fatal(err)
		}
		sources[name] = string(fixture)
	}
	code := tsdiag.Effect_undefined_represents_the_same_outcome_as_Effect_succeed_undefined_or_Effect_succeed_void_0_effect_effectSucceedWithUndefined.Code()
	for name, source := range sources {
		t.Run(name, func(t *testing.T) {
			t.Parallel()
			f, done := fourslash.NewFourslash(t, nil, prefix+source)
			defer done()
			f.VerifyNonSuggestionDiagnostics(t, nil)
			uri := lsconv.FileNameToDocumentURI(tspath.RootedFilePath("/repro.ts"))
			lines := strings.Split(source, "\n")
			checked, diagnostics := 0, 0
			for _, diagnostic := range f.GetQuickFixesForDiagnostics(t, uri) {
				if diagnostic.Diagnostic.Code == nil || diagnostic.Diagnostic.Code.Integer == nil || *diagnostic.Diagnostic.Code.Integer != code {
					continue
				}
				line := lines[diagnostic.Diagnostic.Range.Start.Line]
				wantNoRewrite := commentAboveContains(lines, int(diagnostic.Diagnostic.Range.Start.Line), "NO replacement quickfix")
				offered := false
				for index, title := range diagnostic.FixTitles {
					if title != "Replace with Effect.undefined" {
						continue
					}
					offered = true
					result := f.ApplyQuickFix(t, uri, diagnostic.Diagnostic, index)
					if len(result.Changes) != 1 {
						t.Fatalf("expected one changed file, got %d", len(result.Changes))
					}
					func() {
						converted, closeConverted := fourslash.NewFourslash(t, nil, prefix+result.Changes[0].After)
						defer closeConverted()
						converted.VerifyNonSuggestionDiagnostics(t, nil)
					}()
					checked++
				}
				if offered == wantNoRewrite {
					t.Fatalf("line %q: replacement offered = %v, want %v", line, offered, !wantNoRewrite)
				}
				diagnostics++
			}
			if diagnostics == 0 {
				t.Fatal("no effectSucceedWithUndefined diagnostics were checked")
			}
			t.Logf("compiled %d replacements", checked)
		})
	}
}

// commentAboveContains reports whether the lines of line's case, up to the previous blank line, contain marker.
func commentAboveContains(lines []string, line int, marker string) bool {
	for i := line - 1; i >= 0 && strings.TrimSpace(lines[i]) != ""; i-- {
		if strings.Contains(lines[i], marker) {
			return true
		}
	}
	return false
}
