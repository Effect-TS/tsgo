package effecttest_test

import (
	"strings"
	"testing"

	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/fourslash"
	"github.com/microsoft/TypeScript/tsc/shim/ls/lsconv"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
)

// TestImportStyleFixesCompile applies the preferBarrelImport and
// preferNamespaceImport quick fixes and checks that the result compiles and
// that the fixed import is no longer reported.
func TestImportStyleFixesCompile(t *testing.T) {
	t.Parallel()
	barrel := `"barrelImportPackages": ["effect"], "diagnosticSeverity": { "*": "off", "preferBarrelImport": "suggestion" }`
	namespace := `"namespaceImportPackages": ["effect"], "diagnosticSeverity": { "*": "off", "preferNamespaceImport": "suggestion" }`
	follow := `"namespaceImportPackages": ["effect"], "topLevelNamedReexports": "follow", "diagnosticSeverity": { "*": "off", "preferNamespaceImport": "suggestion" }`
	barrelCode := tsdiag.X_0_is_imported_directly_but_barrelImportPackages_expects_named_imports_from_the_barrel_1_Use_2_instead_effect_preferBarrelImport.Code()
	namespaceCode := tsdiag.X_0_is_imported_by_name_from_1_but_namespaceImportPackages_expects_a_namespace_import_Use_2_instead_effect_preferNamespaceImport.Code()

	cases := []struct {
		name    string
		options string
		code    int32
		source  string
	}{
		{"barrel namespace import", barrel, barrelCode, `import * as R from "effect/Record"
export const keys = R.keys({ a: 1 })
`},
		{"barrel named imports", barrel, barrelCode, `import { empty, set } from "effect/Record"
export const counts = set(empty<string, number>(), "a", 1)
export const factories = { empty }
export type Empty = typeof empty
`},
		{"barrel reuse", barrel, barrelCode, `import { Option } from "effect"
import { some, none } from "effect/Option"
export const values = [some(1), none(), Option.isSome]
`},
		{"barrel type-only", barrel, barrelCode, `import { type Duration } from "effect/Duration"
export type Timeout = Duration
`},
		{"namespace re-export", namespace, namespaceCode, `import { Effect } from "effect"
export const program = Effect.succeed(1)
`},
		{"namespace member", namespace, namespaceCode, `import { map } from "effect/Array"
export const doubled = map([1, 2], (n) => n * 2)
export const helpers = { map }
`},
		{"namespace siblings", namespace, namespaceCode, `import { map, filter } from "effect/Array"
export const evens = filter(map([1, 2, 3], (n) => n * 2), (n) => n > 2)
`},
		{"namespace reuse", namespace, namespaceCode, `import * as Option from "effect/Option"
import { some } from "effect/Option"
export const values = [some(1), Option.none()]
`},
		{"namespace follow", follow, namespaceCode, `import { pipe } from "effect"
export const value = pipe(1, (n: number) => n + 1)
`},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			prefix := `// @filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "skipLibCheck": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [{ "name": "@effect/language-service", ` + tc.options + ` }]
  }
}
// @filename: /repro.ts
`
			uri := lsconv.FileNameToDocumentURI(tspath.RootedFilePath("/repro.ts"))
			count := func(f *fourslash.FourslashTest) int {
				reported := 0
				for _, diagnostic := range f.GetQuickFixesForDiagnostics(t, uri) {
					if diagnostic.Diagnostic.Code != nil && diagnostic.Diagnostic.Code.Integer != nil && *diagnostic.Diagnostic.Code.Integer == tc.code {
						reported++
					}
				}
				return reported
			}

			f, done := fourslash.NewFourslash(t, nil, prefix+tc.source)
			defer done()
			f.VerifyNonSuggestionDiagnostics(t, nil)
			before := count(f)
			checked := 0
			for _, diagnostic := range f.GetQuickFixesForDiagnostics(t, uri) {
				if diagnostic.Diagnostic.Code == nil || diagnostic.Diagnostic.Code.Integer == nil || *diagnostic.Diagnostic.Code.Integer != tc.code {
					continue
				}
				for index, title := range diagnostic.FixTitles {
					if strings.HasPrefix(title, "Disable ") {
						continue
					}
					result := f.ApplyQuickFix(t, uri, diagnostic.Diagnostic, index)
					if len(result.Changes) != 1 {
						t.Fatalf("%q: expected one changed file, got %d", title, len(result.Changes))
					}
					func() {
						fixed, closeFixed := fourslash.NewFourslash(t, nil, prefix+result.Changes[0].After)
						defer closeFixed()
						fixed.VerifyNonSuggestionDiagnostics(t, nil)
						if after := count(fixed); after != before-1 {
							t.Fatalf("%q: expected %d reports after the fix, got %d:\n%s", title, before-1, after, result.Changes[0].After)
						}
					}()
					checked++
				}
			}
			if checked == 0 {
				t.Fatal("no import style fix was applied")
			}
		})
	}
}
