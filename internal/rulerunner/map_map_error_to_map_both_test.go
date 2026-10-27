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

func TestMapMapErrorToMapBothLifecycle(t *testing.T) {
	t.Parallel()
	const prelude = "import { Effect } from \"effect\"\ndeclare const source: Effect.Effect<number, string>\n"
	const pair = "export const value = source.pipe(Effect.map((n) => n + 1), Effect.mapError((e) => e.length))\n"
	const message = ":`Effect.map` + `Effect.mapError` transform the success and failure channels one step at a time; `Effect.mapBoth` expresses both transformations in a single step. effect(mapMapErrorToMapBoth)"
	// The diagnostic covers the second callee, Effect.mapError, of the pair.
	at := func(category int, offset int) string {
		start := offset + len("export const value = source.pipe(Effect.map((n) => n + 1), ")
		return fmt.Sprintf("377153:%d:%d:%d%s", category, start, start+len("Effect.mapError"), message)
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
		{name: "default off", source: prelude + pair, useRule: true},
		{name: "recommended preset", source: prelude + pair, preset: "recommended"},
		{name: "strict preset", source: prelude + pair, preset: "strict"},
		{name: "style preset", source: prelude + pair, preset: "style", want: []string{at(0, base)}},
		{name: "effect-native preset", source: prelude + pair, preset: "effect-native"},
		{name: "warning", source: prelude + pair, severity: etscore.SeverityWarning, want: []string{at(0, base)}},
		{name: "error", source: prelude + pair, severity: etscore.SeverityError, want: []string{at(1, base)}},
		{name: "off", source: prelude + pair},
		{name: "diagnostics disabled", source: prelude + pair, severity: etscore.SeverityWarning, disabled: true},
		{name: "directive disables warning", source: "// @effect-diagnostics mapMapErrorToMapBoth:off\n" + prelude + pair, severity: etscore.SeverityWarning},
		{
			name:     "next line disables one pair",
			source:   prelude + "// @effect-diagnostics-next-line mapMapErrorToMapBoth:off\n" + pair + strings.Replace(pair, "value", "other", 1),
			severity: etscore.SeverityWarning,
			want:     []string{at(0, base+len("// @effect-diagnostics-next-line mapMapErrorToMapBoth:off\n")+len(pair))},
		},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			severity := make(map[string]etscore.Severity)
			for _, r := range rules.All {
				if r.Name != "mapMapErrorToMapBoth" {
					severity[r.Name] = etscore.SeverityOff
				}
			}
			switch {
			case tc.useRule:
				severity["mapMapErrorToMapBoth"] = rules.MapMapErrorToMapBoth.DefaultSeverity
			case tc.preset != "":
				for _, preset := range rules.MetadataPresets() {
					if preset.Name == tc.preset {
						severity["mapMapErrorToMapBoth"] = preset.DiagnosticSeverity["mapMapErrorToMapBoth"]
					}
				}
			default:
				severity["mapMapErrorToMapBoth"] = tc.severity
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
