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

func TestProvideMergeAllToArrayLifecycle(t *testing.T) {
	t.Parallel()
	const prelude = "import { Context, Effect, Layer } from \"effect\"\n" +
		"class A extends Context.Service<A, { readonly a: number }>()(\"A\") {}\n" +
		"class B extends Context.Service<B, { readonly b: number }>()(\"B\") {}\n" +
		"declare const ALayer: Layer.Layer<A>\n" +
		"declare const BLayer: Layer.Layer<B>\n" +
		"declare const program: Effect.Effect<void, never, A | B>\n"
	const provide = "export const value = program.pipe(Effect.provide(Layer.mergeAll(ALayer, BLayer)))\n"
	const message = ":Effect.provide accepts an array of layers and merges them itself; pass the layers as an array instead of wrapping them in Layer.mergeAll. effect(provideMergeAllToArray)"
	// The diagnostic covers the Layer.mergeAll call written as the provide argument.
	at := func(category int, offset int) string {
		start := offset + len("export const value = program.pipe(Effect.provide(")
		return fmt.Sprintf("377161:%d:%d:%d%s", category, start, start+len("Layer.mergeAll(ALayer, BLayer)"), message)
	}
	base := len(prelude)
	cases := []struct {
		name     string
		source   string
		severity etscore.Severity
		useRule  bool
		disabled bool
		preset   string
		want     []string
	}{
		{name: "default off", source: prelude + provide, useRule: true},
		{name: "recommended preset", source: prelude + provide, preset: "recommended"},
		{name: "strict preset", source: prelude + provide, preset: "strict"},
		{name: "style preset", source: prelude + provide, preset: "style", want: []string{at(0, base)}},
		{name: "effect-native preset", source: prelude + provide, preset: "effect-native"},
		{name: "warning", source: prelude + provide, severity: etscore.SeverityWarning, want: []string{at(0, base)}},
		{name: "error", source: prelude + provide, severity: etscore.SeverityError, want: []string{at(1, base)}},
		{name: "off", source: prelude + provide},
		{name: "diagnostics disabled", source: prelude + provide, severity: etscore.SeverityWarning, disabled: true},
		{name: "directive disables warning", source: "// @effect-diagnostics provideMergeAllToArray:off\n" + prelude + provide, severity: etscore.SeverityWarning},
		{
			name:     "next line disables one call",
			source:   prelude + "// @effect-diagnostics-next-line provideMergeAllToArray:off\n" + provide + strings.Replace(provide, "value", "other", 1),
			severity: etscore.SeverityWarning,
			want:     []string{at(0, base+len("// @effect-diagnostics-next-line provideMergeAllToArray:off\n")+len(provide))},
		},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			severity := make(map[string]etscore.Severity)
			for _, r := range rules.All {
				if r.Name != "provideMergeAllToArray" {
					severity[r.Name] = etscore.SeverityOff
				}
			}
			switch {
			case tc.useRule:
				severity["provideMergeAllToArray"] = rules.ProvideMergeAllToArray.DefaultSeverity
			case tc.preset != "":
				for _, preset := range rules.MetadataPresets() {
					if preset.Name == tc.preset {
						severity["provideMergeAllToArray"] = preset.DiagnosticSeverity["provideMergeAllToArray"]
					}
				}
			default:
				severity["provideMergeAllToArray"] = tc.severity
			}
			files := map[string]any{
				"/.src/test.ts": &fstest.MapFile{Data: []byte(tc.source)},
			}
			if err := bundledeffect.MountEffect(bundledeffect.EffectV4, files); err != nil {
				t.Fatal(err)
			}
			fs := bundled.WrapFS(vfstest.FromMap(files, tspath.CaseSensitive))
			options := &core.CompilerOptions{
				Effect:              &etscore.EffectPluginOptions{Diagnostics: !tc.disabled, DiagnosticSeverity: severity},
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
			sourceFile := program.GetSourceFile("/.src/test.ts")
			var got []string
			for _, diagnostic := range append(program.GetSemanticDiagnostics(t.Context(), sourceFile), program.GetSuggestionDiagnostics(t.Context(), sourceFile)...) {
				got = append(got, fmt.Sprintf("%d:%d:%d:%d:%s", diagnostic.Code(), diagnostic.Category(), diagnostic.Pos(), diagnostic.End(), diagnostic.String()))
			}
			if !reflect.DeepEqual(got, tc.want) {
				t.Fatalf("diagnostics = %v, want %v", got, tc.want)
			}
		})
	}
}
