package effecttest_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/effecttest"
	"github.com/microsoft/TypeScript/tsc/shim/fourslash"
	"github.com/microsoft/TypeScript/tsc/shim/ls/lsconv"
)

func TestEffectFnOpportunityUnaryPipeablesCompile(t *testing.T) {
	t.Parallel()

	fixture, err := os.ReadFile(filepath.Join(effecttest.TestDataPath(), "tests", "effect-v4", "effectFnOpportunity_unaryPipeables.ts"))
	if err != nil {
		t.Fatal(err)
	}
	_, source, found := strings.Cut(string(fixture), "// @filename: effectFnOpportunity_unaryPipeables.ts\n")
	if !found {
		t.Fatal("missing fixture source")
	}
	const prefix = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [{
      "name": "@effect/language-service",
      "effectFn": ["span", "untraced", "no-span", "inferred-span", "suggested-span"]
    }]
  }
}
// @Filename: /repro.ts
`
	f, done := fourslash.NewFourslash(t, nil, prefix+source)
	defer done()
	f.VerifyNonSuggestionDiagnostics(t, nil)

	uri := lsconv.FileNameToDocumentURI("/repro.ts")
	checked := 0
	for _, diagnostic := range f.GetQuickFixesForDiagnostics(t, uri) {
		for index, title := range diagnostic.FixTitles {
			if !strings.HasPrefix(title, "Convert to Effect.fn") {
				continue
			}
			result := f.ApplyQuickFix(t, uri, diagnostic.Diagnostic, index)
			if len(result.Changes) != 1 {
				t.Fatalf("%s: expected one changed file, got %d", title, len(result.Changes))
			}
			t.Logf("Checking %s at %v", title, diagnostic.Diagnostic.Range)
			func() {
				converted, closeConverted := fourslash.NewFourslash(t, nil, prefix+result.Changes[0].After)
				defer closeConverted()
				converted.VerifyNonSuggestionDiagnostics(t, nil)
			}()
			checked++
		}
	}
	if checked == 0 {
		t.Fatal("no Effect.fn conversions were checked")
	}
}
