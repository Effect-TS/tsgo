package effecttest_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/effecttest"
	"github.com/microsoft/TypeScript/tsc/shim/fourslash"
	"github.com/microsoft/TypeScript/tsc/shim/ls/lsconv"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
)

type fixCompileCase struct {
	fixture string
	matches int
	fixes   int
}

type fixesCompileFile struct {
	name    string
	content string
}

type fixesCompileMatch struct {
	file int
	info fourslash.QuickFixInfo
}

func verifyFixesCompile(t *testing.T, version bundledeffect.EffectVersion, ruleName string, code int32, isFix func(title string) bool, cases []fixCompileCase) {
	t.Helper()
	prefix := `// @filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "skipLibCheck": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [{
      "name": "@effect/language-service",
      "diagnosticSeverity": { "*": "off", "promiseInEffectSuccess": "off", "` + ruleName + `": "suggestion" }
    }]
  }
}
`
	compile := func(t *testing.T, files []fixesCompileFile) (*fourslash.FourslashTest, func(), []fixesCompileMatch) {
		t.Helper()
		var sb strings.Builder
		sb.WriteString(prefix)
		for _, file := range files {
			sb.WriteString("// @filename: " + file.name + "\n" + file.content)
		}
		f, done := fourslash.NewFourslash(t, nil, sb.String())
		var matches []fixesCompileMatch
		for index, file := range files {
			f.GoToFile(t, file.name)
			f.VerifyNonSuggestionDiagnostics(t, nil)
			for _, info := range f.GetQuickFixesForDiagnostics(t, lsconv.FileNameToDocumentURI(tspath.RootedFilePath(file.name))) {
				if info.Diagnostic.Code != nil && info.Diagnostic.Code.Integer != nil && *info.Diagnostic.Code.Integer == code {
					matches = append(matches, fixesCompileMatch{file: index, info: info})
				}
			}
		}
		return f, done, matches
	}
	for _, test := range cases {
		t.Run(string(version)+"/"+test.fixture, func(t *testing.T) {
			t.Parallel()
			content, err := os.ReadFile(filepath.Join(effecttest.TestDataPath(), "tests", string(version), test.fixture))
			if err != nil {
				t.Fatal(err)
			}
			source := demoteRuleToSuggestion(string(content), ruleName)
			if !strings.HasPrefix(source, "// @filename:") {
				source = "// @filename: repro.ts\n" + source
			}
			var files []fixesCompileFile
			for _, line := range strings.SplitAfter(source, "\n") {
				if name, ok := strings.CutPrefix(line, "// @filename:"); ok {
					files = append(files, fixesCompileFile{name: "/" + strings.TrimSpace(name), content: "// @" + string(version) + "\n"})
					continue
				}
				files[len(files)-1].content += line
			}
			f, done, matches := compile(t, files)
			defer done()
			fixes := 0
			for _, match := range matches {
				uri := lsconv.FileNameToDocumentURI(tspath.RootedFilePath(files[match.file].name))
				for index, title := range match.info.FixTitles {
					if !isFix(title) {
						continue
					}
					result := f.ApplyQuickFix(t, uri, match.info.Diagnostic, index)
					if len(result.Changes) != 1 || result.Changes[0].URI != uri {
						t.Fatalf("expected one change in %s, got %d", uri, len(result.Changes))
					}
					fixed := append([]fixesCompileFile(nil), files...)
					fixed[match.file].content = result.Changes[0].After
					func() {
						_, closeFixed, remaining := compile(t, fixed)
						defer closeFixed()
						if len(remaining) != len(matches)-1 {
							t.Fatalf("expected %d diagnostics after %q, got %d", len(matches)-1, title, len(remaining))
						}
					}()
					fixes++
				}
			}
			if len(matches) != test.matches || fixes != test.fixes {
				t.Fatalf("got %d diagnostics and %d compiled fixes, want %d and %d", len(matches), fixes, test.matches, test.fixes)
			}
		})
	}
}

func demoteRuleToSuggestion(source string, ruleName string) string {
	return strings.ReplaceAll(source, ruleName+":warning", ruleName+":suggestion")
}
