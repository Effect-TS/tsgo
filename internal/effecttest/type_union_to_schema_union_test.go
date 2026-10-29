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

func TestTypeUnionToSchemaUnionFixesCompile(t *testing.T) {
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
      "diagnosticSeverity": { "*": "off", "typeUnionToSchemaUnion": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
`
			// union spells Schema.Union arguments for the version under test.
			union := func(args string) string {
				if version == bundledeffect.EffectV4 {
					return "[" + args + "]"
				}
				return args
			}
			type fixCase struct {
				name     string
				source   string
				expected string
				matches  int
				fixes    int
				// unresolved marks fixes that intentionally reference an unbound
				// Schema; only their text is checked.
				unresolved bool
			}
			cases := []fixCase{
				{
					name: "exported",
					source: `import { Schema } from "effect"
export const A = Schema.Struct({ a: Schema.String })
export const B = Schema.Struct({ b: Schema.String })
export type AB = typeof A.Type | typeof B.Type
`,
					expected: `import { Schema } from "effect"
export const A = Schema.Struct({ a: Schema.String })
export const B = Schema.Struct({ b: Schema.String })
export const AB = Schema.Union(` + union("A, B") + `);
export type AB = typeof AB.Type
`,
					matches: 1,
					fixes:   1,
				},
				{
					// The const goes before the JSDoc, which stays attached to the alias.
					name: "jsDoc",
					source: `import * as S from "effect/Schema"
const A = S.String
const B = S.Boolean
/** Either A or B */
type AB = typeof A.Encoded | typeof B.Encoded
export const value: AB = "a"
`,
					expected: `import * as S from "effect/Schema"
const A = S.String
const B = S.Boolean
const AB = S.Union(` + union("A, B") + `);
/** Either A or B */
type AB = typeof AB.Encoded
export const value: AB = "a"
`,
					matches: 1,
					fixes:   1,
				},
				{
					name: "namespace",
					source: `import { Schema } from "effect"
const A = Schema.String
const B = Schema.Boolean
export namespace Models {
  export type AB = typeof A.Type | typeof B.Type
}
`,
					expected: `import { Schema } from "effect"
const A = Schema.String
const B = Schema.Boolean
export namespace Models {
  export const AB = Schema.Union(` + union("A, B") + `);
  export type AB = typeof AB.Type
}
`,
					matches: 1,
					fixes:   1,
				},
				{
					// Code before the alias on its line is not indentation: the const
					// is inserted inline instead of repeating `function f() { `.
					name: "inlineFunctionBody",
					source: `import { Schema } from "effect"
const A = Schema.String
const B = Schema.Boolean
export function f() { type AB = typeof A.Type | typeof B.Type; return [] as Array<AB> }
`,
					expected: `import { Schema } from "effect"
const A = Schema.String
const B = Schema.Boolean
export function f() { const AB = Schema.Union(` + union("A, B") + `); type AB = typeof AB.Type; return [] as Array<AB> }
`,
					matches: 1,
					fixes:   1,
				},
				{
					name: "sameLineStatement",
					source: `import { Schema } from "effect"
const A = Schema.String
const B = Schema.Boolean
export const n = 1; export type AB = typeof A.Encoded | typeof B.Encoded
`,
					expected: `import { Schema } from "effect"
const A = Schema.String
const B = Schema.Boolean
export const n = 1; export const AB = Schema.Union(` + union("A, B") + `); export type AB = typeof AB.Encoded
`,
					matches: 1,
					fixes:   1,
				},
				{
					// A const inserted before the alias would use B before its declaration.
					name: "lateMember",
					source: `import { Schema } from "effect"
const A = Schema.String
export type AB = typeof A.Type | typeof B.Type
const B = Schema.Boolean
export const value: AB = "a"
`,
					matches: 1,
				},
				{
					// A qualified member whose namespace is declared later is late too.
					name: "lateNamespace",
					source: `import { Schema } from "effect"
export type AB = typeof Models.A.Type | typeof Models.B.Type
namespace Models {
  export const A = Schema.String
  export const B = Schema.Boolean
}
export const value: AB = "a"
`,
					matches: 1,
				},
				{
					// No Schema module binding: the fix falls back to Schema and adds no import.
					name: "fallbackReference",
					source: `import { Boolean as SchemaBoolean, String as SchemaString } from "effect/Schema"
export type AB = typeof SchemaString.Type | typeof SchemaBoolean.Type
`,
					expected: `import { Boolean as SchemaBoolean, String as SchemaString } from "effect/Schema"
export const AB = Schema.Union(` + union("SchemaString, SchemaBoolean") + `);
export type AB = typeof AB.Type
`,
					matches:    1,
					fixes:      1,
					unresolved: true,
				},
			}
			if version == bundledeffect.EffectV4 {
				cases = append(cases,
					fixCase{name: "typeUnionToSchemaUnion.ts", matches: 23, fixes: 21},
					fixCase{name: "typeUnionToSchemaUnion_aliasedImport.ts", matches: 4, fixes: 4},
				)
			} else {
				cases = append(cases,
					fixCase{name: "typeUnionToSchemaUnion.ts", matches: 23, fixes: 21},
					fixCase{name: "typeUnionToSchemaUnion_aliasedImport.ts", matches: 3, fixes: 3},
				)
			}
			cases = append(cases, fixCase{name: "typeUnionToSchemaUnion_preview.ts", matches: 1, fixes: 1})

			code := tsdiag.This_type_alias_unions_Schema_0_types_without_a_runtime_schema_compose_them_with_Schema_Union_and_derive_the_type_from_it_effect_typeUnionToSchemaUnion.Code()
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
						source = strings.Replace(string(fixture), "typeUnionToSchemaUnion:warning", "typeUnionToSchemaUnion:suggestion", 1)
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
							if title != "Convert to Schema.Union" {
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
							if !test.unresolved {
								func() {
									converted, closeConverted := fourslash.NewFourslash(t, nil, prefix+after)
									defer closeConverted()
									converted.GoToFile(t, "/repro.ts")
									converted.VerifyNonSuggestionDiagnostics(t, nil)
									if remaining := countMatches(converted); remaining != test.matches-1 {
										t.Fatalf("expected %d diagnostics after replacement, got %d", test.matches-1, remaining)
									}
								}()
							}
							fixes++
						}
					}
					if matches != test.matches || fixes != test.fixes {
						t.Fatalf("got %d diagnostics and %d fixes, want %d and %d", matches, fixes, test.matches, test.fixes)
					}
				})
			}
		})
	}
}
