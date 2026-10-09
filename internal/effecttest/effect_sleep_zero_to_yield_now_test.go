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

func TestEffectSleepZeroToYieldNowFixesCompile(t *testing.T) {
	t.Parallel()
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			for _, name := range []string{"effectSleepZeroToYieldNow.ts", "effectSleepZeroToYieldNow_aliases.ts", "effectSleepZeroToYieldNow_preview.ts"} {
				fixture, err := os.ReadFile(filepath.Join(effecttest.TestDataPath(), "tests", string(version), name))
				if err != nil {
					t.Fatal(err)
				}
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
      "diagnosticSeverity": { "*": "off", "effectSleepZeroToYieldNow": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
`
				f, done := fourslash.NewFourslash(t, nil, prefix+string(fixture))
				f.VerifyNonSuggestionDiagnostics(t, nil)
				uri := lsconv.FileNameToDocumentURI(tspath.RootedFilePath("/repro.ts"))
				inventory := f.GetQuickFixesForDiagnostics(t, uri)
				code := tsdiag.Consider_Effect_yieldNow_for_cooperative_yielding_instead_of_sleeping_for_zero_duration_effect_effectSleepZeroToYieldNow.Code()
				suggestions := 0
				for _, diagnostic := range inventory {
					if diagnostic.Diagnostic.Code != nil && diagnostic.Diagnostic.Code.Integer != nil && *diagnostic.Diagnostic.Code.Integer == code {
						suggestions++
					}
				}
				checked := 0
				for _, diagnostic := range inventory {
					for index, title := range diagnostic.FixTitles {
						if title != "Replace with Effect.yieldNow" {
							continue
						}
						result := f.ApplyQuickFix(t, uri, diagnostic.Diagnostic, index)
						if len(result.Changes) != 1 {
							t.Fatalf("%s: expected one changed file, got %d", name, len(result.Changes))
						}
						func() {
							converted, closeConverted := fourslash.NewFourslash(t, nil, prefix+result.Changes[0].After)
							defer closeConverted()
							converted.VerifyNonSuggestionDiagnostics(t, nil)
							remaining := converted.GetQuickFixesForDiagnostics(t, uri)
							count := 0
							for _, diagnostic := range remaining {
								if diagnostic.Diagnostic.Code != nil && diagnostic.Diagnostic.Code.Integer != nil && *diagnostic.Diagnostic.Code.Integer == code {
									count++
								}
							}
							if count != suggestions-1 {
								t.Fatalf("%s: expected %d sleep-zero diagnostics after replacement, got %d", name, suggestions-1, count)
							}
						}()
						checked++
					}
				}
				done()
				if checked == 0 {
					t.Fatalf("%s: no yieldNow replacements were checked", name)
				}
				t.Logf("%s: compiled %d replacements", name, checked)
			}
		})
	}
}
