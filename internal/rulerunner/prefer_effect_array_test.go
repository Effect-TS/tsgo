package rulerunner_test

import (
	"fmt"
	"reflect"
	"testing"
	"testing/fstest"

	_ "github.com/effect-ts/tsgo/etscheckerhooks"
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/bundled"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

func TestPreferEffectArrayLifecycle(t *testing.T) {
	t.Parallel()
	const source = "const xs = [1]; xs.map(n => n);"
	const message = "Consider effect/Array.map instead of native Array.map. The callback type accepts (value, index), with no array parameter or thisArg. effect(preferEffectArray)"
	const isArrayMessage = "Consider effect/Array.isArray instead of native Array.isArray. Narrows elements to unknown rather than any. effect(preferEffectArray)"
	cases := []struct {
		name     string
		source   string
		severity etscore.Severity
		want     []string
	}{
		{name: "default off", source: source},
		{name: "warning", source: source, severity: etscore.SeverityWarning, want: []string{"377139:0:19:22:" + message}},
		{name: "error", source: source, severity: etscore.SeverityError, want: []string{"377139:1:19:22:" + message}},
		{name: "dot reference", source: "const xs = [1]; const alias = xs.map; alias.call(xs, n => n);", severity: etscore.SeverityWarning, want: []string{"377139:0:33:36:" + message}},
		{name: "bracket invocation", source: "const xs = [1]; xs[\"map\"](n => n);", severity: etscore.SeverityWarning, want: []string{"377139:0:19:24:" + message}},
		{name: "bracket reference", source: "const xs = [1]; const alias = xs[\"map\"]; alias.call(xs, n => n);", severity: etscore.SeverityWarning, want: []string{"377139:0:33:38:" + message}},
		{name: "constructor dot", source: "Array.isArray([]);", severity: etscore.SeverityWarning, want: []string{"377139:0:6:13:" + isArrayMessage}},
		{name: "constructor bracket", source: "Array[\"isArray\"]([]);", severity: etscore.SeverityWarning, want: []string{"377139:0:6:15:" + isArrayMessage}},
		{name: "optional bracket", source: "declare const xs: number[] | undefined; xs?.[\"map\"](n => n);", severity: etscore.SeverityWarning, want: []string{"377139:0:45:50:" + message}},
		{name: "generic bracket invocation", source: "function constrained<T extends readonly number[]>(values: T) { values[\"map\"](n => n); }", severity: etscore.SeverityWarning, want: []string{"377139:0:70:75:" + message}},
		{name: "generic bracket reference", source: "function constrained<T extends readonly number[]>(values: T) { return values[\"map\"]; }", severity: etscore.SeverityWarning, want: []string{"377139:0:77:82:" + message}},
		{name: "custom and mixed generic brackets", source: `function custom<T extends { map: (f: (n: number) => number) => number[] }>(values: T) { values["map"]; }
function mixed<T extends readonly number[] | { map: (f: (n: number) => number) => number[] }>(values: T) { values["map"]; }
const xs = [1]; xs.map(n => n);`, severity: etscore.SeverityWarning, want: []string{"377139:0:248:251:" + message}},
		{name: "escaped and template names", source: "const xs = [1]; xs.m\\u0061p(n => n); xs[\"m\\u0061p\"](n => n); xs[`map`](n => n);", severity: etscore.SeverityWarning, want: []string{"377139:0:19:27:" + message, "377139:0:40:50:" + message, "377139:0:64:69:" + message}},
		{name: "parenthesized delete target", source: "declare const xs: Partial<Pick<number[], \"map\">>; delete (xs.map); xs.map;", severity: etscore.SeverityWarning, want: []string{"377139:0:70:73:" + message}},
		{name: "directive enables default off", source: "// @effect-diagnostics preferEffectArray:warning\n" + source, want: []string{"377139:0:68:71:" + message}},
		{name: "directive disables warning", source: "// @effect-diagnostics preferEffectArray:off\n" + source, severity: etscore.SeverityWarning},
		{name: "next line disables one reference", source: "// @effect-diagnostics-next-line preferEffectArray:off\n" + source + "\nxs.map(n => n);", severity: etscore.SeverityWarning, want: []string{"377139:0:90:93:" + message}},
		{name: "next line enables default off", source: "// @effect-diagnostics-next-line preferEffectArray:error\n" + source + "\nxs.map(n => n);", want: []string{"377139:1:76:79:" + message}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			severity := make(map[string]etscore.Severity)
			for _, r := range rules.All {
				if r.Name != "preferEffectArray" {
					severity[r.Name] = etscore.SeverityOff
				}
			}
			if tc.severity != etscore.SeverityOff {
				severity["preferEffectArray"] = tc.severity
			}
			fs := bundled.WrapFS(vfstest.FromMap(map[string]any{
				"/.src/test.ts": &fstest.MapFile{Data: []byte(tc.source)},
			}, tspath.CaseSensitive))
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
			if !reflect.DeepEqual(got, tc.want) {
				t.Fatalf("diagnostics = %v, want %v", got, tc.want)
			}
		})
	}
}
