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

func TestPreferEffectRecordLifecycle(t *testing.T) {
	t.Parallel()
	const keysMessage = "Consider effect/Record.keys instead of native Object.keys. Requires a record rather than arrays or primitives; types keys as the inferred key union, which does not guarantee exact runtime keys. effect(preferEffectRecord)"
	const valuesMessage = "Consider effect/Record.values instead of native Object.values. Requires a record rather than arrays or primitives; snapshots keys before reads, so getter or proxy mutations can change the result. effect(preferEffectRecord)"
	const entriesMessage = "Consider effect/Record.toEntries instead of native Object.entries. Requires a record rather than arrays or primitives; types entries with inferred keys and snapshots keys before reads, so getter or proxy mutations can change the result. effect(preferEffectRecord)"
	const fromEntriesMessage = "Consider effect/Record.fromEntries instead of native Object.fromEntries. Requires typed string or symbol key tuples; native numeric keys and loose entry arrays need adaptation. effect(preferEffectRecord)"
	const hasMessage = "Consider effect/Record.has instead of native Object.hasOwn. Requires a record rather than arrays or primitives and a string or symbol key of that record's key type; arbitrary strings may need narrowing. effect(preferEffectRecord)"
	const prototypeMessage = "Consider effect/Record.has instead of native Object.prototype.hasOwnProperty. Pass the receiver explicitly; requires a record rather than arrays or primitives and a string or symbol key of that record's key type. Avoids instance binding and overrides. effect(preferEffectRecord)"
	cases := []struct {
		name     string
		source   string
		severity etscore.Severity
		disabled bool
		preset   string
		want     []string
	}{
		{name: "default off", source: "Object.keys({ a: 1 });"},
		{name: "effect native preset", source: "Object.keys({ a: 1 });", preset: "effect-native", want: []string{"377143:0:7:11:" + keysMessage}},
		{name: "recommended preset", source: "Object.keys({ a: 1 });", preset: "recommended"},
		{name: "strict preset", source: "Object.keys({ a: 1 });", preset: "strict"},
		{name: "warning", source: "Object.keys({ a: 1 });", severity: etscore.SeverityWarning, want: []string{"377143:0:7:11:" + keysMessage}},
		{name: "error", source: "Object.keys({ a: 1 });", severity: etscore.SeverityError, want: []string{"377143:1:7:11:" + keysMessage}},
		{name: "diagnostics disabled", source: "Object.keys({ a: 1 });", severity: etscore.SeverityWarning, disabled: true},
		{name: "dot reference", source: "const alias = Object.keys; alias({ a: 1 });", severity: etscore.SeverityWarning, want: []string{"377143:0:21:25:" + keysMessage}},
		{name: "bracket invocation", source: "Object[\"entries\"]({ a: 1 });", severity: etscore.SeverityWarning, want: []string{"377143:0:7:16:" + entriesMessage}},
		{name: "bracket reference", source: "const alias = Object[\"keys\"]; alias({ a: 1 });", severity: etscore.SeverityWarning, want: []string{"377143:0:21:27:" + keysMessage}},
		{name: "values", source: "Object.values({ a: 1 });", severity: etscore.SeverityWarning, want: []string{"377143:0:7:13:" + valuesMessage}},
		{name: "fromEntries", source: "Object.fromEntries([[1, \"a\"]]);", severity: etscore.SeverityWarning, want: []string{"377143:0:7:18:" + fromEntriesMessage}},
		{name: "hasOwn", source: "Object.hasOwn({ a: 1 }, \"a\");", severity: etscore.SeverityWarning, want: []string{"377143:0:7:13:" + hasMessage}},
		{name: "borrowed prototype", source: "Object.prototype.hasOwnProperty.call({ a: 1 }, \"a\");", severity: etscore.SeverityWarning, want: []string{"377143:0:17:31:" + prototypeMessage}},
		{name: "inherited method", source: "const data = { a: 1 }; data.hasOwnProperty(\"a\");", severity: etscore.SeverityWarning, want: []string{"377143:0:28:42:" + prototypeMessage}},
		{name: "optional bracket", source: "declare const native: ObjectConstructor | undefined; native?.[\"keys\"]({ a: 1 });", severity: etscore.SeverityWarning, want: []string{"377143:0:62:68:" + keysMessage}},
		{name: "generic bracket", source: "function constrained<T extends ObjectConstructor>(native: T) { return native[\"keys\"]; }", severity: etscore.SeverityWarning, want: []string{"377143:0:77:83:" + keysMessage}},
		{name: "array and primitive arguments", source: "Object.keys([1]); Object.values(\"a\"); Object.hasOwn([], \"a\");", severity: etscore.SeverityWarning, want: []string{"377143:0:7:11:" + keysMessage, "377143:0:25:31:" + valuesMessage, "377143:0:45:51:" + hasMessage}},
		{name: "escaped and template names", source: "Object.k\\u0065ys({ a: 1 }); Object[\"k\\u0065ys\"]({ a: 1 }); Object[`keys`]({ a: 1 });", severity: etscore.SeverityWarning, want: []string{"377143:0:7:16:" + keysMessage, "377143:0:35:46:" + keysMessage, "377143:0:66:72:" + keysMessage}},
		{name: "parenthesized delete target", source: "declare const native: Partial<Pick<ObjectConstructor, \"keys\">>; delete (native.keys); native.keys;", severity: etscore.SeverityWarning, want: []string{"377143:0:93:97:" + keysMessage}},
		{name: "type write and destructuring positions", source: "type Keys = typeof Object.keys; Object.keys = () => []; ({ keys: Object.keys } = Object); const { keys } = Object; Object.keys({ a: 1 });", severity: etscore.SeverityWarning, want: []string{"377143:0:122:126:" + keysMessage}},
		{name: "super access", source: "class Base {} class Child extends Base { test() { return super.hasOwnProperty(\"a\"); } } Object.keys({ a: 1 });", severity: etscore.SeverityWarning, want: []string{"377143:0:95:99:" + keysMessage}},
		{name: "shadow and mixed roots", source: "function shadowed(Object: { keys(value: object): string[] }) { return Object.keys({}); } declare const mixed: ObjectConstructor | { keys(value: object): string[] }; mixed.keys({}); Object.keys({ a: 1 });", severity: etscore.SeverityWarning, want: []string{"377143:0:188:192:" + keysMessage}},
		{name: "directive enables default off", source: "// @effect-diagnostics preferEffectRecord:warning\nObject.keys({ a: 1 });", want: []string{"377143:0:57:61:" + keysMessage}},
		{name: "directive disables warning", source: "// @effect-diagnostics preferEffectRecord:off\nObject.keys({ a: 1 });", severity: etscore.SeverityWarning},
		{name: "next line disables one reference", source: "// @effect-diagnostics-next-line preferEffectRecord:off\nObject.keys({ a: 1 });\nObject.keys({ a: 1 });", severity: etscore.SeverityWarning, want: []string{"377143:0:86:90:" + keysMessage}},
		{name: "next line enables default off", source: "// @effect-diagnostics-next-line preferEffectRecord:error\nObject.keys({ a: 1 });\nObject.keys({ a: 1 });", want: []string{"377143:1:65:69:" + keysMessage}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			severity := make(map[string]etscore.Severity)
			for _, r := range rules.All {
				if r.Name != "preferEffectRecord" {
					severity[r.Name] = etscore.SeverityOff
				}
			}
			if tc.severity != etscore.SeverityOff {
				severity["preferEffectRecord"] = tc.severity
			}
			if tc.preset != "" {
				for _, preset := range rules.MetadataPresets() {
					if preset.Name == tc.preset {
						severity["preferEffectRecord"] = preset.DiagnosticSeverity["preferEffectRecord"]
					}
				}
			}
			fs := bundled.WrapFS(vfstest.FromMap(map[string]any{
				"/.src/test.ts": &fstest.MapFile{Data: []byte(tc.source)},
			}, tspath.CaseSensitive))
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
