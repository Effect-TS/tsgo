package rulerunner_test

import (
	"fmt"
	"reflect"
	"strings"
	"testing"
	"testing/fstest"

	_ "github.com/effect-ts/tsgo/etscheckerhooks"
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/bundled"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

func TestTypeUnionToSchemaUnionLifecycle(t *testing.T) {
	t.Parallel()
	const ruleName = "typeUnionToSchemaUnion"
	const message = "This type alias unions Schema `.Type` types without a runtime schema; compose them with `Schema.Union` and derive the type from it. effect(typeUnionToSchemaUnion)"
	const body = "import { Schema } from \"effect\"\nexport const A = Schema.String\nexport const B = Schema.Boolean\nexport type AB = typeof A.Type | typeof B.Type\n"
	const twice = body + "export type BA = typeof B.Type | typeof A.Type\n"
	nextLine := strings.Replace(twice, "export type AB", "// @effect-diagnostics-next-line typeUnionToSchemaUnion:off\nexport type AB", 1)
	// expect formats the diagnostic on the alias name of the n-th type alias of source.
	expect := func(source string, n int, category int) string {
		start := -1
		for range n {
			start += 1 + strings.Index(source[start+1:], "export type ")
		}
		start += len("export type ")
		return fmt.Sprintf("377159:%d:%d:%d:%s", category, start, start+len("AB"), message)
	}
	const (
		categoryWarning = 0
		categoryError   = 1
	)
	cases := []struct {
		name     string
		source   string
		severity etscore.Severity
		preset   string
		want     []string
	}{
		{name: "default off", source: body},
		{name: "recommended preset", source: body, preset: "recommended"},
		{name: "style preset", source: body, preset: "style", want: []string{expect(body, 1, categoryWarning)}},
		{name: "strict preset", source: body, preset: "strict"},
		{name: "correctness preset", source: body, preset: "correctness"},
		{name: "warning", source: body, severity: etscore.SeverityWarning, want: []string{expect(body, 1, categoryWarning)}},
		{name: "error", source: body, severity: etscore.SeverityError, want: []string{expect(body, 1, categoryError)}},
		{name: "directive disables", source: "// @effect-diagnostics typeUnionToSchemaUnion:off\n" + body, severity: etscore.SeverityWarning},
		{name: "directive enables", source: "// @effect-diagnostics typeUnionToSchemaUnion:error\n" + body, want: []string{expect("// @effect-diagnostics typeUnionToSchemaUnion:error\n"+body, 1, categoryError)}},
		{name: "next line disables one alias", source: nextLine, severity: etscore.SeverityWarning, want: []string{expect(nextLine, 2, categoryWarning)}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			severity := make(map[string]etscore.Severity)
			for _, r := range rules.All {
				if r.Name != ruleName {
					severity[r.Name] = etscore.SeverityOff
				}
			}
			if tc.severity != etscore.SeverityOff {
				severity[ruleName] = tc.severity
			}
			if tc.preset != "" {
				found := false
				for _, preset := range rules.MetadataPresets() {
					if preset.Name == tc.preset {
						found = true
						if s, ok := preset.DiagnosticSeverity[ruleName]; ok {
							severity[ruleName] = s
						} else {
							severity[ruleName] = etscore.SeverityOff
						}
					}
				}
				if !found {
					t.Fatalf("preset %s not found", tc.preset)
				}
			}
			files := map[string]any{"/.src/test.ts": &fstest.MapFile{Data: []byte(tc.source)}}
			if err := bundledeffect.MountEffect(bundledeffect.EffectV4, files); err != nil {
				t.Fatal(err)
			}
			fs := bundled.WrapFS(vfstest.FromMap(files, tspath.CaseSensitive))
			options := &core.CompilerOptions{
				Effect:              &etscore.EffectPluginOptions{Diagnostics: true, DiagnosticSeverity: severity},
				Target:              core.ScriptTargetESNext,
				Module:              core.ModuleKindNodeNext,
				ModuleResolution:    core.ModuleResolutionKindNodeNext,
				Strict:              core.TSTrue,
				SkipDefaultLibCheck: core.TSTrue,
				SkipLibCheck:        core.TSTrue,
			}
			program := compiler.NewProgram(compiler.ProgramOptions{
				Config:         tsoptions.NewParsedCommandLine(options, []tspath.RootedFilePath{"/.src/test.ts"}, nil, "/.src", tspath.CaseSensitive),
				Host:           compiler.NewCompilerHost(fs, bundled.LibPath(), nil, nil, nil),
				SingleThreaded: core.TSTrue,
			})
			var got []string
			for _, diagnostic := range program.GetSemanticDiagnostics(t.Context(), nil) {
				got = append(got, fmt.Sprintf("%d:%d:%d:%d:%s", diagnostic.Code(), diagnostic.Category(), diagnostic.Pos(), diagnostic.End(), diagnostic.String()))
			}
			for _, diagnostic := range program.GetSuggestionDiagnostics(t.Context(), nil) {
				got = append(got, fmt.Sprintf("%d:%d:%d:%d:%s", diagnostic.Code(), diagnostic.Category(), diagnostic.Pos(), diagnostic.End(), diagnostic.String()))
			}
			if !reflect.DeepEqual(got, tc.want) {
				t.Fatalf("diagnostics = %v, want %v", got, tc.want)
			}
		})
	}
}
