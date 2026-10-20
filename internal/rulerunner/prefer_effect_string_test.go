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

func TestPreferEffectStringLifecycle(t *testing.T) {
	t.Parallel()
	cases := []struct {
		name     string
		source   string
		severity etscore.Severity
		disabled bool
		preset   string
		want     []string
	}{
		{name: "default off", source: "\"hello\".trim();"},
		{name: "effect-native preset", source: "\"hello\".trim();", preset: "effect-native", want: []string{"377150:0:8:12:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
		{name: "recommended preset", source: "\"hello\".trim();", preset: "recommended"},
		{name: "strict preset", source: "\"hello\".trim();", preset: "strict"},
		{name: "style preset", source: "\"hello\".trim();", preset: "style"},
		{name: "warning", source: "\"hello\".trim();", severity: etscore.SeverityWarning, want: []string{"377150:0:8:12:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
		{name: "error", source: "\"hello\".trim();", severity: etscore.SeverityError, want: []string{"377150:1:8:12:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
		{name: "diagnostics disabled", source: "\"hello\".trim();", severity: etscore.SeverityWarning, disabled: true},
		{name: "at", source: "\"hello\".at(0);", severity: etscore.SeverityWarning, want: []string{"377150:0:8:10:Consider effect/String.at instead of native String.at. Returns Option, with None replacing undefined. effect(preferEffectString)"}},
		{name: "charAt", source: "\"hello\".charAt(0);", severity: etscore.SeverityWarning, want: []string{"377150:0:8:14:Consider effect/String.charAt instead of native String.charAt. Returns Option, with None replacing the empty string for an out-of-range index. effect(preferEffectString)"}},
		{name: "charCodeAt", source: "\"hello\".charCodeAt(0);", severity: etscore.SeverityWarning, want: []string{"377150:0:8:18:Consider effect/String.charCodeAt instead of native String.charCodeAt. Returns Option<number>, with None replacing NaN. effect(preferEffectString)"}},
		{name: "codePointAt", source: "\"hello\".codePointAt(0);", severity: etscore.SeverityWarning, want: []string{"377150:0:8:19:Consider effect/String.codePointAt instead of native String.codePointAt. Returns Option<number>, with None replacing undefined. effect(preferEffectString)"}},
		{name: "concat", source: "\"hello\".concat(\"!\");", severity: etscore.SeverityWarning, want: []string{"377150:0:8:14:Consider effect/String.concat instead of native String.concat. Accepts exactly one string argument; native concat accepts any number of arguments and converts non-strings. effect(preferEffectString)"}},
		{name: "endsWith", source: "\"hello\".endsWith(\"o\");", severity: etscore.SeverityWarning, want: []string{"377150:0:8:16:Consider effect/String.endsWith instead of native String.endsWith. Same runtime behavior. effect(preferEffectString)"}},
		{name: "includes", source: "\"hello\".includes(\"e\");", severity: etscore.SeverityWarning, want: []string{"377150:0:8:16:Consider effect/String.includes instead of native String.includes. Same runtime behavior. effect(preferEffectString)"}},
		{name: "indexOf", source: "\"hello\".indexOf(\"l\");", severity: etscore.SeverityWarning, want: []string{"377150:0:8:15:Consider effect/String.indexOf instead of native String.indexOf. Returns Option<number>, with None replacing -1; there is no position parameter. effect(preferEffectString)"}},
		{name: "lastIndexOf", source: "\"hello\".lastIndexOf(\"l\");", severity: etscore.SeverityWarning, want: []string{"377150:0:8:19:Consider effect/String.lastIndexOf instead of native String.lastIndexOf. Returns Option<number>, with None replacing -1; there is no position parameter. effect(preferEffectString)"}},
		{name: "localeCompare", source: "\"hello\".localeCompare(\"world\");", severity: etscore.SeverityWarning, want: []string{"377150:0:8:21:Consider effect/String.localeCompare instead of native String.localeCompare. Returns an Ordering of -1, 0, or 1; Effect v4 types locales as Array<string>, so a single locale string needs an array. effect(preferEffectString)"}},
		{name: "match", source: "\"hello\".match(/l/);", severity: etscore.SeverityWarning, want: []string{"377150:0:8:13:Consider effect/String.match instead of native String.match. Returns Option, with None replacing null. effect(preferEffectString)"}},
		{name: "matchAll", source: "\"hello\".matchAll(/l/g);", severity: etscore.SeverityWarning, want: []string{"377150:0:8:16:Consider effect/String.matchAll instead of native String.matchAll. Requires a RegExp; string patterns are not accepted. effect(preferEffectString)"}},
		{name: "normalize", source: "\"hello\".normalize();", severity: etscore.SeverityWarning, want: []string{"377150:0:8:17:Consider effect/String.normalize instead of native String.normalize. Same runtime behavior. effect(preferEffectString)"}},
		{name: "padEnd", source: "\"hello\".padEnd(8);", severity: etscore.SeverityWarning, want: []string{"377150:0:8:14:Consider effect/String.padEnd instead of native String.padEnd. Same runtime behavior. effect(preferEffectString)"}},
		{name: "padStart", source: "\"hello\".padStart(8);", severity: etscore.SeverityWarning, want: []string{"377150:0:8:16:Consider effect/String.padStart instead of native String.padStart. Same runtime behavior. effect(preferEffectString)"}},
		{name: "repeat", source: "\"hello\".repeat(2);", severity: etscore.SeverityWarning, want: []string{"377150:0:8:14:Consider effect/String.repeat instead of native String.repeat. In Effect v4, a negative count returns an empty string instead of throwing RangeError. effect(preferEffectString)"}},
		{name: "replace", source: "\"hello\".replace(\"l\", \"r\");", severity: etscore.SeverityWarning, want: []string{"377150:0:8:15:Consider effect/String.replace instead of native String.replace. Requires a string replacement; replacer callbacks are not accepted. effect(preferEffectString)"}},
		{name: "replaceAll", source: "\"hello\".replaceAll(\"l\", \"r\");", severity: etscore.SeverityWarning, want: []string{"377150:0:8:18:Consider effect/String.replaceAll instead of native String.replaceAll. Requires a string replacement; replacer callbacks are not accepted. effect(preferEffectString)"}},
		{name: "search", source: "\"hello\".search(/l/);", severity: etscore.SeverityWarning, want: []string{"377150:0:8:14:Consider effect/String.search instead of native String.search. Returns Option<number>, with None replacing -1. effect(preferEffectString)"}},
		{name: "slice", source: "\"hello\".slice(1);", severity: etscore.SeverityWarning, want: []string{"377150:0:8:13:Consider effect/String.slice instead of native String.slice. Same runtime behavior. effect(preferEffectString)"}},
		{name: "split", source: "\"hello\".split(\"\");", severity: etscore.SeverityWarning, want: []string{"377150:0:8:13:Consider effect/String.split instead of native String.split. Returns a NonEmptyArray, so splitting an empty string by \"\" returns [\"\"] instead of []; there is no limit parameter. effect(preferEffectString)"}},
		{name: "startsWith", source: "\"hello\".startsWith(\"h\");", severity: etscore.SeverityWarning, want: []string{"377150:0:8:18:Consider effect/String.startsWith instead of native String.startsWith. Same runtime behavior. effect(preferEffectString)"}},
		{name: "substring", source: "\"hello\".substring(1);", severity: etscore.SeverityWarning, want: []string{"377150:0:8:17:Consider effect/String.substring instead of native String.substring. Same runtime behavior. effect(preferEffectString)"}},
		{name: "toLocaleLowerCase", source: "\"hello\".toLocaleLowerCase();", severity: etscore.SeverityWarning, want: []string{"377150:0:8:25:Consider effect/String.toLocaleLowerCase instead of native String.toLocaleLowerCase. Same runtime behavior; Effect v4 types the locale as string or Array<string>, without Intl.Locale. effect(preferEffectString)"}},
		{name: "toLocaleUpperCase", source: "\"hello\".toLocaleUpperCase();", severity: etscore.SeverityWarning, want: []string{"377150:0:8:25:Consider effect/String.toLocaleUpperCase instead of native String.toLocaleUpperCase. Same runtime behavior; Effect v4 types the locale as string or Array<string>, without Intl.Locale. effect(preferEffectString)"}},
		{name: "toLowerCase", source: "\"hello\".toLowerCase();", severity: etscore.SeverityWarning, want: []string{"377150:0:8:19:Consider effect/String.toLowerCase instead of native String.toLowerCase. Same runtime behavior; literal inputs keep a Lowercase<T> type. effect(preferEffectString)"}},
		{name: "toUpperCase", source: "\"hello\".toUpperCase();", severity: etscore.SeverityWarning, want: []string{"377150:0:8:19:Consider effect/String.toUpperCase instead of native String.toUpperCase. Same runtime behavior; literal inputs keep an Uppercase<T> type. effect(preferEffectString)"}},
		{name: "trim", source: "\"hello\".trim();", severity: etscore.SeverityWarning, want: []string{"377150:0:8:12:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
		{name: "trimStart", source: "\"hello\".trimStart();", severity: etscore.SeverityWarning, want: []string{"377150:0:8:17:Consider effect/String.trimStart instead of native String.trimStart. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
		{name: "trimEnd", source: "\"hello\".trimEnd();", severity: etscore.SeverityWarning, want: []string{"377150:0:8:15:Consider effect/String.trimEnd instead of native String.trimEnd. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
		{name: "trimLeft", source: "\"hello\".trimLeft();", severity: etscore.SeverityWarning, want: []string{"377150:0:8:16:Consider effect/String.trimStart instead of native String.trimLeft. Deprecated alias of trimStart; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
		{name: "trimRight", source: "\"hello\".trimRight();", severity: etscore.SeverityWarning, want: []string{"377150:0:8:17:Consider effect/String.trimEnd instead of native String.trimRight. Deprecated alias of trimEnd; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
		{name: "bracket invocation", source: "\"hello\"[\"trim\"]();", severity: etscore.SeverityWarning, want: []string{"377150:0:8:14:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
		{name: "parenthesized invocation", source: "(\"hello\".trim)();", severity: etscore.SeverityWarning, want: []string{"377150:0:9:13:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
		{name: "optional call", source: "\"hello\".trim?.();", severity: etscore.SeverityWarning, want: []string{"377150:0:8:12:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
		{name: "optional receiver", source: "declare const s: string | undefined; s?.trim();", severity: etscore.SeverityWarning, want: []string{"377150:0:40:44:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
		{name: "optional bracket", source: "declare const s: string | undefined; s?.[\"trim\"]();", severity: etscore.SeverityWarning, want: []string{"377150:0:41:47:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
		{name: "template name", source: "\"hello\"[`trim`]();", severity: etscore.SeverityWarning, want: []string{"377150:0:8:14:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
		{name: "escaped name", source: "\"hello\".tr\\u0069m();", severity: etscore.SeverityWarning, want: []string{"377150:0:8:17:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
		{name: "uncalled reference", source: "const f = \"hello\".trim; f();", severity: etscore.SeverityWarning},
		{name: "callback reference", source: "[\"hello\"].map(\"hello\".trim);", severity: etscore.SeverityWarning},
		{name: "borrowed call", source: "String.prototype.trim.call(\"hello\");", severity: etscore.SeverityWarning},
		{name: "excluded members", source: "\"hello\".substr(1); \"hello\".toString(); \"hello\".anchor(\"id\"); \"hello\".isWellFormed();", severity: etscore.SeverityWarning},
		{name: "other excluded members", source: "\"hello\".valueOf(); \"hello\".toWellFormed(); \"hello\"[Symbol.iterator](); \"hello\".length;", severity: etscore.SeverityWarning},
		{name: "constructor members", source: "String.fromCharCode(65); String.fromCodePoint(65); String.raw`hello`;", severity: etscore.SeverityWarning},
		{name: "mixed roots", source: "declare const mixed: string | Array<string>; mixed.includes(\"h\"); mixed.slice(1);", severity: etscore.SeverityWarning},
		{name: "any receiver", source: "declare const s: any; s.trim();", severity: etscore.SeverityWarning},
		{name: "augmentation", source: "export {}; declare global { interface String { trim(): string; } } \"hello\".trim();", severity: etscore.SeverityWarning},
		{name: "super receiver", source: "class S extends String { test() { return super.trim(); } }", severity: etscore.SeverityWarning},
		{name: "writes and delete", source: "declare let s: Partial<Pick<String, \"trim\">>; s.trim = () => \"\"; delete (s.trim); const { trim } = s;", severity: etscore.SeverityWarning},
		{name: "directive enables default off", source: "// @effect-diagnostics preferEffectString:warning\n\"hello\".trim();", want: []string{"377150:0:58:62:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
		{name: "directive disables warning", source: "// @effect-diagnostics preferEffectString:off\n\"hello\".trim();", severity: etscore.SeverityWarning},
		{name: "next line disables one invocation", source: "// @effect-diagnostics-next-line preferEffectString:off\n\"hello\".trim();\n\"hello\".trim();", severity: etscore.SeverityWarning, want: []string{"377150:0:80:84:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
		{name: "next line enables default off", source: "// @effect-diagnostics-next-line preferEffectString:error\n\"hello\".trim();\n\"hello\".trim();", want: []string{"377150:1:66:70:Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			severity := make(map[string]etscore.Severity)
			for _, r := range rules.All {
				if r.Name != "preferEffectString" {
					severity[r.Name] = etscore.SeverityOff
				}
			}
			if tc.severity != etscore.SeverityOff {
				severity["preferEffectString"] = tc.severity
			}
			if tc.preset != "" {
				for _, preset := range rules.MetadataPresets() {
					if preset.Name == tc.preset {
						severity["preferEffectString"] = preset.DiagnosticSeverity["preferEffectString"]
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
