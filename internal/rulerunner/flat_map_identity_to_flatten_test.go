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

func TestFlatMapIdentityToFlattenLifecycle(t *testing.T) {
	t.Parallel()
	const ruleName = "flatMapIdentityToFlatten"
	const message = "`Effect.flatten` expresses this unwrapping more directly than `Effect.flatMap` with an identity callback. effect(flatMapIdentityToFlatten)"
	const body = "import { Effect } from \"effect\"\ndeclare const loadTask: Effect.Effect<Effect.Effect<string>>\nexport const program = loadTask.pipe(Effect.flatMap((inner) => inner))\n"
	const twice = body + "export const again = loadTask.pipe(Effect.flatMap((inner) => inner))\n"
	// expect formats the diagnostic on the n-th Effect.flatMap of source.
	expect := func(source string, n int, category int) string {
		start := -1
		for range n {
			start += 1 + strings.Index(source[start+1:], "Effect.flatMap")
		}
		return fmt.Sprintf("377151:%d:%d:%d:%s", category, start, start+len("Effect.flatMap"), message)
	}
	const (
		categoryWarning    = 0
		categoryError      = 1
		categorySuggestion = 2
	)
	cases := []struct {
		name     string
		source   string
		severity etscore.Severity
		preset   string
		want     []string
	}{
		{name: "default suggestion", source: body, want: []string{expect(body, 1, categorySuggestion)}},
		{name: "recommended preset", source: body, preset: "recommended", want: []string{expect(body, 1, categorySuggestion)}},
		{name: "style preset", source: body, preset: "style", want: []string{expect(body, 1, categoryWarning)}},
		{name: "strict preset", source: body, preset: "strict", want: []string{expect(body, 1, categoryError)}},
		{name: "correctness preset", source: body, preset: "correctness"},
		{name: "warning", source: body, severity: etscore.SeverityWarning, want: []string{expect(body, 1, categoryWarning)}},
		{name: "error", source: body, severity: etscore.SeverityError, want: []string{expect(body, 1, categoryError)}},
		{name: "directive disables", source: "// @effect-diagnostics flatMapIdentityToFlatten:off\n" + body},
		{
			name:   "next line disables one step",
			source: strings.Replace(twice, "export const program", "// @effect-diagnostics-next-line flatMapIdentityToFlatten:off\nexport const program", 1),
			want: []string{expect(strings.Replace(twice, "export const program", "// @effect-diagnostics-next-line flatMapIdentityToFlatten:off\nexport const program", 1), 2, categorySuggestion)},
		},
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
