package rulerunner_test

import (
	"context"
	"fmt"
	"reflect"
	"slices"
	"sort"
	"strings"
	"testing"
	"testing/fstest"

	_ "github.com/effect-ts/tsgo/etscheckerhooks"
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/bundled"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

// apiStabilityLifecycleOptions builds the effect plugin options with only the
// API stability leak rule enabled at warning severity (or disabled entirely
// for the control run).
func apiStabilityLifecycleOptions(enabled bool) *etscore.EffectPluginOptions {
	severity := map[string]etscore.Severity{}
	for _, r := range rules.All {
		severity[r.Name] = etscore.SeverityOff
	}
	if enabled {
		severity["apiStabilityLeak"] = etscore.SeverityWarning
	}
	return &etscore.EffectPluginOptions{Diagnostics: true, DiagnosticSeverity: severity}
}

// apiStabilityLifecycleDiagnostics runs a virtual multi-file project through
// the real after-check callback path (etscheckerhooks -> rulerunner) and returns
// the diagnostics reported for the project, including global (file-less)
// diagnostics such as TS2589. With wholeProgram set it asks the program for the
// semantic diagnostics; otherwise it asks the checker for the file diagnostics,
// which is the lifecycle the language service uses. Globals are collected on
// both paths: a rule-triggered instantiation guard reports TS2589 as a global,
// and omitting globals would hide it from the enabled/disabled comparison.
func apiStabilityLifecycleDiagnostics(t *testing.T, files map[string]string, enabled bool, wholeProgram bool) []string {
	t.Helper()

	fsmap := map[string]any{}
	names := []tspath.RootedFilePath{}
	for name, source := range files {
		path := "/.src/" + name
		fsmap[path] = &fstest.MapFile{Data: []byte(source)}
		names = append(names, tspath.RootedFilePath(path))
	}
	slices.Sort(names)

	fs := bundled.WrapFS(vfstest.FromMap(fsmap, tspath.CaseSensitive))
	options := &core.CompilerOptions{
		Effect:              apiStabilityLifecycleOptions(enabled),
		Target:              core.ScriptTargetESNext,
		Module:              core.ModuleKindNodeNext,
		ModuleResolution:    core.ModuleResolutionKindNodeNext,
		Strict:              core.TSTrue,
		SkipLibCheck:        core.TSTrue,
		SkipDefaultLibCheck: core.TSTrue,
	}
	host := compiler.NewCompilerHost(fs, bundled.LibPath(), nil, nil, nil)
	program := compiler.NewProgram(compiler.ProgramOptions{
		Config:         tsoptions.NewParsedCommandLine(options, names, nil, "/.src", tspath.CaseSensitive),
		Host:           host,
		SingleThreaded: core.TSTrue,
	})
	ctx := context.Background()
	sf := program.GetSourceFile("/.src/test.ts")
	if sf == nil {
		t.Fatal("missing test.ts")
	}

	var messages []string
	add := func(diagnostic *ast.Diagnostic) {
		if diagnostic == nil {
			return
		}
		if diagnosticFile := diagnostic.File(); diagnosticFile != nil {
			messages = append(messages, string(diagnosticFile.FileName())+":"+diagnostic.String())
			return
		}
		messages = append(messages, diagnostic.String())
	}
	if wholeProgram {
		for _, diagnostic := range program.GetSemanticDiagnostics(ctx, nil) {
			add(diagnostic)
		}
		for _, diagnostic := range program.GetGlobalDiagnostics(ctx) {
			add(diagnostic)
		}
	} else {
		checker, done := program.GetTypeChecker(ctx)
		for _, diagnostic := range checker.GetDiagnostics(ctx, sf) {
			add(diagnostic)
		}
		for _, diagnostic := range checker.GetGlobalDiagnostics() {
			add(diagnostic)
		}
		done()
	}
	return messages
}

func apiStabilityLifecycleContains(messages []string, want string) bool {
	for _, message := range messages {
		if strings.Contains(message, want) {
			return true
		}
	}
	return false
}

// apiStabilityLifecycleCases covers the lazy-read lifecycle findings under the
// corrected boundary. Each case runs through the production after-check
// callback with normal diagnostics, in both the per-file and whole-program
// lifecycles, and compares against a control run with the rule disabled so a
// compiler diagnostic introduced by the rule cannot hide.
func TestApiStabilityLeakLifecycle(t *testing.T) {
	t.Parallel()
	prefix := "/** @stability experimental */\ninterface E {value:string}\ndeclare const e:E\n"

	cases := []struct {
		name    string
		files   map[string]string
		want    []string
		notWant []string
	}{
		{
			name:  "anonymous inferred conditional return",
			files: map[string]string{"test.ts": prefix + `declare function make<T>():{value:T extends string ? number:E}; export const x=make<number>()`},
			want:  []string{"`x` exposes `E`"},
		},
		{
			name:  "inferred callable conditional return",
			files: map[string]string{"test.ts": prefix + `declare function make<T>():()=>T extends string ? number:E; export const x=make<number>()`},
			want:  []string{"`x` exposes `E`"},
		},
		{
			name:  "nested anonymous conditional return",
			files: map[string]string{"test.ts": prefix + `declare function make<T>():{f:()=>{value:T extends string?number:E}}; export const x=make<number>()`},
			want:  []string{"`x` exposes `E`"},
		},
		{
			name:  "contravariant conditional member",
			files: map[string]string{"test.ts": prefix + `interface C<in T>{f:(x:T)=>void}; interface Holder<T>{value:T extends C<string> ? number:E}; export declare const x:Holder<C<"x">>`},
			want:  []string{"`x` exposes `E`"},
		},
		{
			name:  "invariant conditional member",
			files: map[string]string{"test.ts": prefix + `interface C<in out T>{f:(x:T)=>T}; interface Holder<T>{value:T extends C<string> ? number:E}; export declare const x:Holder<C<"x">>`},
			want:  []string{"`x` exposes `E`"},
		},
		{
			name:  "array wrapped conditional member",
			files: map[string]string{"test.ts": prefix + `interface C<in T>{f:(x:T)=>void}; interface Holder<T>{value:T extends C<string>[] ? number:E}; export declare const x:Holder<C<"x">[]>`},
			want:  []string{"`x` exposes `E`"},
		},
		{
			name:    "structural conditional member stays clean",
			files:   map[string]string{"test.ts": prefix + `interface Holder<T>{value:T extends {value:string} ? number:E}; export declare const x:Holder<{value:string}>`},
			notWant: []string{"`x` exposes `E`"},
		},
		{
			name:  "represented indexed callable return",
			files: map[string]string{"test.ts": prefix + `interface I{value:E}; type Read<T>=T["value" & keyof T]; declare function make<T>():()=>Read<T>; export const x=make<I>(); x()`},
			want:  []string{"`x` exposes `E`"},
		},
		{
			name:  "lazily materialized indexed callable return",
			files: map[string]string{"test.ts": prefix + `interface I{value:E}; type Read<T>=T["value" & keyof T]; declare function make<T>():()=>Read<T>; export const x=make<I>()`},
			want:  []string{"`x` exposes `E`"},
		},
		{
			name: "single-file barrel ts dependency",
			files: map[string]string{
				"test.ts": `export {f} from "./dep.js"`,
				"dep.ts":  prefix + `export declare function f(x:E):void`,
			},
			want: []string{"`f` exposes `E`"},
		},
		{
			name: "single-file barrel dts dependency",
			files: map[string]string{
				"test.ts":  `export {f} from "./dep.js"`,
				"dep.d.ts": prefix + `export declare function f(x:E):void`,
			},
			want: []string{"`f` exposes `E`"},
		},
		{
			name: "single-file star barrel dts dependency",
			files: map[string]string{
				"test.ts":  `export * from "./dep.js"`,
				"dep.d.ts": prefix + `export interface X {value:E}`,
			},
			want: []string{"`X` exposes `E`"},
		},
		{
			name:    "shallow nested named reference stays quiet",
			files:   map[string]string{"test.ts": prefix + `interface Hidden<T>{value:E}; declare const h:Hidden<string>; h.value; export interface X{hidden:Hidden<string>}`},
			notWant: []string{"`X` exposes `E`"},
		},
		{
			name:    "shallow nested tagged reference stays quiet",
			files:   map[string]string{"test.ts": prefix + `interface Hidden<T>{/** @stability experimental */ value:string}; declare const h:Hidden<string>; h.value; export interface X{hidden:Hidden<string>}`},
			notWant: []string{"`X` exposes `E`"},
		},
		{
			name:  "erased alias in collapsed union",
			files: map[string]string{"test.ts": "/** @stability experimental */\ntype A=string\nexport declare const x: never|number|A"},
			want:  []string{"`x` exposes `A`"},
		},
		{
			name:  "erased alias in reordered union",
			files: map[string]string{"test.ts": "/** @stability experimental */\ntype A=string\nexport declare const x: number|A|never"},
			want:  []string{"`x` exposes `A`"},
		},
		{
			name:  "erased alias in collapsed intersection",
			files: map[string]string{"test.ts": "/** @stability experimental */\ntype A=string\nexport declare const x: A&string"},
			want:  []string{"`x` exposes `A`"},
		},
		{
			name:  "declared conditional alias branch",
			files: map[string]string{"test.ts": prefix + `export type X<T> = T extends string ? number : E`},
			want:  []string{"`X` exposes `E`"},
		},
		{
			name: "finite recursive alias application",
			files: map[string]string{"test.ts": prefix + `type A<T> = T extends string ? A<number> : E
export declare const x: A<string>`},
			want: []string{"`x` exposes `E`"},
		},
		{
			name: "finite recursive tuple alias application",
			files: map[string]string{"test.ts": prefix + `type Step<T extends unknown[]> = T extends [unknown, ...infer Tail] ? Step<Tail> : E
export declare const x: Step<[0, 0]>`},
			want: []string{"`x` exposes `E`"},
		},
		{
			name:  "declared index signature tag",
			files: map[string]string{"test.ts": "export interface I {\n/** @stability experimental */\n[key:string]:number\n}"},
			want:  []string{"`I` exposes `index signature`"},
		},
	}

	for _, test := range cases {
		for _, wholeProgram := range []bool{false, true} {
			mode := "file"
			if wholeProgram {
				mode = "program"
			}
			t.Run(test.name+"/"+mode, func(t *testing.T) {
				t.Parallel()
				messages := apiStabilityLifecycleDiagnostics(t, test.files, true, wholeProgram)
				control := apiStabilityLifecycleDiagnostics(t, test.files, false, wholeProgram)
				for _, want := range test.want {
					if !apiStabilityLifecycleContains(messages, want) {
						t.Errorf("missing %q in %v", want, messages)
					}
				}
				for _, notWant := range test.notWant {
					if apiStabilityLifecycleContains(messages, notWant) {
						t.Errorf("unexpected %q in %v", notWant, messages)
					}
				}
				// The rule must not introduce compiler diagnostics on
				// semantically valid code: every non-rule message must also be
				// present in the control. Rule messages carry the effect code.
				for _, message := range messages {
					if strings.Contains(message, "effect(apiStabilityLeak)") {
						continue
					}
					if !apiStabilityLifecycleContains(control, message) {
						t.Errorf("rule introduced compiler diagnostic %q\nrule=%v\ncontrol=%v", message, messages, control)
					}
				}
			})
		}
	}
}

// TestApiStabilityLeakExportVisibilityLifecycle runs the corrected
// export-level visibility policy through the production after-check callback in
// both lifecycles: the explicit export winner beats a redundant star forward,
// declared namespaces filter tagged members, and import-then-export aliases
// follow their import target. Public peers and public targets still warn, and
// the disabled-rule control stays diagnostic-free, so no compiler diagnostic is
// introduced by the metadata decisions.
func TestApiStabilityLeakExportVisibilityLifecycle(t *testing.T) {
	t.Parallel()
	prefix := "/** @stability experimental */\ninterface E { value: string }\n/** @stability experimental */\ninterface F { value: string }\n"

	cases := []struct {
		name    string
		files   map[string]string
		want    []string
		notWant []string
	}{
		{
			name: "tagged named export plus redundant star",
			files: map[string]string{
				"test.ts": "/** @internal */\nexport { Leaky } from \"./dep\"\nexport * from \"./dep\"\n",
				"dep.ts":  prefix + "export interface Leaky { value: E }\n",
			},
			// dep.ts's own direct export of Leaky is public there and is
			// legitimately checked; only the tagged test.ts re-export is
			// expected to stay quiet.
			notWant: []string{"/.src/test.ts:`Leaky` exposes"},
		},
		{
			name: "tagged local export plus redundant cyclic star",
			files: map[string]string{
				"test.ts": prefix + "/** @internal */\nexport interface Leaky { value: E }\nexport * from \"./dep\"\n",
				"dep.ts":  "export { Leaky } from \"./test\"\n",
			},
			notWant: []string{"`Leaky` exposes"},
		},
		{
			name: "public local export plus tagged redundant star stays checked",
			files: map[string]string{
				"test.ts": prefix + "export interface Leaky { value: E }\n/** @internal */\nexport * from \"./dep\"\n",
				"dep.ts":  "export * from \"./test\"\n",
			},
			want: []string{"`Leaky` exposes `E`"},
		},
		{
			name: "declared namespace filters tagged member",
			files: map[string]string{
				"test.ts": prefix + "export namespace Api {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: string }\n}\n",
			},
			notWant: []string{"`Api` exposes `E`"},
		},
		{
			name: "declared namespace public peer still checked",
			files: map[string]string{
				"test.ts": prefix + "export namespace Api {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: F }\n}\n",
			},
			want:    []string{"`Api` exposes `F`"},
			notWant: []string{"`Api` exposes `E`"},
		},
		{
			name: "import then export follows internal named target",
			files: map[string]string{
				"test.ts": "import { Hidden } from \"./dep\"\nexport { Hidden }\n",
				"dep.ts":  prefix + "/** @internal */\nexport interface Hidden { value: E }\n",
			},
			notWant: []string{"`Hidden` exposes"},
		},
		{
			name: "default import then export default follows internal target",
			files: map[string]string{
				"test.ts": "import Hidden from \"./dep\"\nexport default Hidden\n",
				"dep.ts":  prefix + "/** @internal */\nexport default function hidden(): E { return null as unknown as E }\n",
			},
			notWant: []string{"`default` exposes"},
		},
		{
			name: "public import then export still warns",
			files: map[string]string{
				"test.ts": "import { Shown } from \"./dep\"\nexport { Shown }\n",
				"dep.ts":  prefix + "export interface Shown { value: E }\n",
			},
			want: []string{"`Shown` exposes `E`"},
		},
		{
			name: "internal and public peers keep independent verdicts",
			files: map[string]string{
				"test.ts": prefix + "/** @internal */\nexport declare const aHidden: E\nexport declare const zShown: E\n",
			},
			want:    []string{"`zShown` exposes `E`"},
			notWant: []string{"`aHidden` exposes"},
		},
	}

	for _, test := range cases {
		for _, wholeProgram := range []bool{false, true} {
			mode := "file"
			if wholeProgram {
				mode = "program"
			}
			t.Run(test.name+"/"+mode, func(t *testing.T) {
				t.Parallel()
				control := apiStabilityLifecycleDiagnostics(t, test.files, false, wholeProgram)
				if len(control) != 0 {
					t.Fatalf("invalid control: %v", control)
				}
				messages := apiStabilityLifecycleDiagnostics(t, test.files, true, wholeProgram)
				for _, want := range test.want {
					if !apiStabilityLifecycleContains(messages, want) {
						t.Errorf("missing %q in %v", want, messages)
					}
				}
				for _, notWant := range test.notWant {
					if apiStabilityLifecycleContains(messages, notWant) {
						t.Errorf("unexpected %q in %v", notWant, messages)
					}
				}
				for _, message := range messages {
					if strings.Contains(message, "effect(apiStabilityLeak)") {
						continue
					}
					t.Errorf("rule introduced compiler diagnostic %q\nrule=%v\ncontrol=%v", message, messages, control)
				}
				if strings.Contains(strings.Join(messages, "\n"), "TS2589") {
					t.Errorf("rule introduced TS2589: %v", messages)
				}
			})
		}
	}
}

// TestApiStabilityLeakImportEqualsLifecycle pins the import-equals metadata
// decision in the production after-check lifecycle: an `export import X = ...` alias follows
// the target named by its module reference, so the tagged target stays quiet in
// both the file and whole-program hook paths while its untagged peer is checked,
// and the disabled-rule control stays diagnostic-free. The same decision is
// exercised for a namespace qualifier, a named import of that namespace and an
// external `require`.
func TestApiStabilityLeakImportEqualsLifecycle(t *testing.T) {
	t.Parallel()
	prefix := "/** @stability experimental */\ninterface E { value: string }\n"

	cases := []struct {
		name    string
		files   map[string]string
		want    []string
		notWant []string
	}{
		{
			name: "local namespace import equals follows internal target",
			files: map[string]string{
				"test.ts": prefix + "namespace Impl {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: E }\n}\nexport import Hidden = Impl.Hidden\nexport import Shown = Impl.Shown\n",
			},
			want:    []string{"`Shown` exposes `E`"},
			notWant: []string{"`Hidden` exposes"},
		},
		{
			name: "cross file namespace import equals follows internal target",
			files: map[string]string{
				"test.ts": "import { Impl } from \"./dep\"\nexport import Hidden = Impl.Hidden\nexport import Shown = Impl.Shown\n",
				"dep.ts":  prefix + "export namespace Impl {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: E }\n}\n",
			},
			want:    []string{"`Shown` exposes `E`"},
			notWant: []string{"`Hidden` exposes"},
		},
		{
			name: "external require import equals follows internal member",
			files: map[string]string{
				"test.ts": "import Dep = require(\"./dep\")\nexport import Hidden = Dep.Hidden\nexport import Shown = Dep.Shown\n",
				"dep.ts":  prefix + "/** @internal */\nexport interface Hidden { value: E }\nexport interface Shown { value: E }\n",
			},
			want:    []string{"`Shown` exposes `E`"},
			notWant: []string{"`Hidden` exposes"},
		},
	}

	for _, test := range cases {
		for _, wholeProgram := range []bool{false, true} {
			mode := "file"
			if wholeProgram {
				mode = "program"
			}
			t.Run(test.name+"/"+mode, func(t *testing.T) {
				t.Parallel()
				control := apiStabilityLifecycleDiagnostics(t, test.files, false, wholeProgram)
				if len(control) != 0 {
					t.Fatalf("invalid control: %v", control)
				}
				messages := apiStabilityLifecycleDiagnostics(t, test.files, true, wholeProgram)
				for _, want := range test.want {
					if !apiStabilityLifecycleContains(messages, want) {
						t.Errorf("missing %q in %v", want, messages)
					}
				}
				for _, notWant := range test.notWant {
					if apiStabilityLifecycleContains(messages, notWant) {
						t.Errorf("unexpected %q in %v", notWant, messages)
					}
				}
				for _, message := range messages {
					if strings.Contains(message, "effect(apiStabilityLeak)") {
						continue
					}
					t.Errorf("rule introduced compiler diagnostic %q\nrule=%v\ncontrol=%v", message, messages, control)
				}
				if strings.Contains(strings.Join(messages, "\n"), "TS2589") {
					t.Errorf("rule introduced TS2589: %v", messages)
				}
			})
		}
	}
}

// TestApiStabilityLeakVisibilityLifecycleRecursiveSafety keeps the
// recursive-alias budget of the visibility fixtures: the star/cycle and
// namespace shapes used by the regressions must add no compiler
// diagnostics to a program whose checking is unaffected, including the repeated
// star cycle.
func TestApiStabilityLeakVisibilityLifecycleRecursiveSafety(t *testing.T) {
	t.Parallel()
	deepAlias := "type Deep<T> = T extends string ? Deep<T> : T\n"

	files := map[string]string{
		"test.ts": deepAlias + "/** @internal */\nexport interface Cyclic { value: Deep<string> }\nexport * from \"./dep\"\n",
		"dep.ts":  "export * from \"./test\"\n",
	}
	for _, wholeProgram := range []bool{false, true} {
		mode := "file"
		if wholeProgram {
			mode = "program"
		}
		t.Run(mode, func(t *testing.T) {
			t.Parallel()
			messages := apiStabilityLifecycleDiagnostics(t, files, true, wholeProgram)
			control := apiStabilityLifecycleDiagnostics(t, files, false, wholeProgram)
			for _, message := range messages {
				if strings.Contains(message, "effect(apiStabilityLeak)") {
					continue
				}
				if !apiStabilityLifecycleContains(control, message) {
					t.Errorf("rule introduced compiler diagnostic %q\nrule=%v\ncontrol=%v", message, messages, control)
				}
			}
			if apiStabilityLifecycleContains(messages, "`Cyclic` exposes") {
				t.Errorf("tagged cyclic export was checked: %v", messages)
			}
		})
	}
}

// TestApiStabilityLeakLifecycleRecursiveSafety keeps the recursive-alias safety
// property in the production lifecycle: a recursive generic alias is never
// expanded by the rule, so the rule adds no TS2589 to a program whose own
// checking does not produce one, and a program that already produces one keeps
// exactly its baseline diagnostics.
func TestApiStabilityLeakLifecycleRecursiveSafety(t *testing.T) {
	t.Parallel()
	deepAlias := "type Deep<T> = T extends string ? Deep<T> : T\n"

	t.Run("member budget adds no diagnostic", func(t *testing.T) {
		t.Parallel()
		files := map[string]string{"test.ts": deepAlias + `interface Wrapper<T> { value: Deep<T> }
export declare const exposed: Wrapper<string>
`}
		messages := apiStabilityLifecycleDiagnostics(t, files, true, true)
		control := apiStabilityLifecycleDiagnostics(t, files, false, true)
		for _, message := range messages {
			if strings.Contains(message, "effect(apiStabilityLeak)") {
				continue
			}
			if !apiStabilityLifecycleContains(control, message) {
				t.Errorf("rule introduced compiler diagnostic %q\nrule=%v\ncontrol=%v", message, messages, control)
			}
		}
	})

	t.Run("computed name baseline preserved", func(t *testing.T) {
		t.Parallel()
		files := map[string]string{"test.ts": deepAlias + `declare const key: Deep<string>
export interface X { [key]: number }
`}
		messages := apiStabilityLifecycleDiagnostics(t, files, true, true)
		control := apiStabilityLifecycleDiagnostics(t, files, false, true)
		// The baseline already reports TS2589 for this intentionally invalid
		// input; the rule must not change that diagnostic set.
		if len(messages) != len(control) {
			t.Errorf("rule changed diagnostics for an already-erroring Deep input:\nrule=%v\ncontrol=%v", messages, control)
		}
		if len(control) == 0 {
			t.Fatal("expected the control to report the baseline TS2589")
		}
	})
}

// apiStabilityRecursivePrefix declares the recursive alias used by the recursive-alias
// safety regressions. `Deep<T>` keeps expanding itself so any speculative
// resolution of an application of it would trip the checker's depth guard.
const apiStabilityRecursivePrefix = "type Deep<T> = T extends string ? Deep<T> : T\n"

// runApiStabilitySafetyCase runs one project through the production after-check
// callback in the per-file and whole-program lifecycles, with the rule enabled
// and disabled. It asserts that the rule changes no compiler diagnostic on
// either lifecycle in either direction (added or lost, globals included, so a
// rule-provoked TS2589 cannot hide and a suppressed ordinary diagnostic cannot
// disappear) and checks the expected and forbidden rule diagnostics.
func runApiStabilitySafetyCase(t *testing.T, name string, files map[string]string, want, notWant []string) {
	t.Helper()
	for _, wholeProgram := range []bool{false, true} {
		mode := "file"
		if wholeProgram {
			mode = "program"
		}
		t.Run(name+"/"+mode, func(t *testing.T) {
			t.Parallel()
			messages := apiStabilityLifecycleDiagnostics(t, files, true, wholeProgram)
			control := apiStabilityLifecycleDiagnostics(t, files, false, wholeProgram)
			for _, message := range messages {
				if strings.Contains(message, "effect(apiStabilityLeak)") {
					continue
				}
				if !apiStabilityLifecycleContains(control, message) {
					t.Errorf("rule introduced compiler diagnostic %q\nrule=%v\ncontrol=%v", message, messages, control)
				}
			}
			for _, message := range control {
				if !apiStabilityLifecycleContains(messages, message) {
					t.Errorf("rule lost compiler diagnostic %q\nrule=%v\ncontrol=%v", message, messages, control)
				}
			}
			for _, expected := range want {
				if !apiStabilityLifecycleContains(messages, expected) {
					t.Errorf("missing %q in %v", expected, messages)
				}
			}
			for _, forbidden := range notWant {
				if apiStabilityLifecycleContains(messages, forbidden) {
					t.Errorf("unexpected %q in %v", forbidden, messages)
				}
			}
		})
	}
}

// apiStabilityProgram builds one virtual project and returns its program
// and checker without running any rule. The caller owns the returned cleanup,
// which releases the checker; sharing one checker across a sequence of queries
// is what the later-read regressions need.
func apiStabilityProgram(t *testing.T, files map[string]string, enabled bool) (*compiler.Program, *checker.Checker, func()) {
	t.Helper()
	fsmap := map[string]any{}
	var names []tspath.RootedFilePath
	for name, source := range files {
		path := "/.src/" + name
		fsmap[path] = &fstest.MapFile{Data: []byte(source)}
		names = append(names, tspath.RootedFilePath(path))
	}
	slices.Sort(names)
	fs := bundled.WrapFS(vfstest.FromMap(fsmap, tspath.CaseSensitive))
	options := &core.CompilerOptions{
		Effect:              apiStabilityLifecycleOptions(enabled),
		Target:              core.ScriptTargetESNext,
		Module:              core.ModuleKindNodeNext,
		ModuleResolution:    core.ModuleResolutionKindNodeNext,
		Strict:              core.TSTrue,
		SkipLibCheck:        core.TSTrue,
		SkipDefaultLibCheck: core.TSTrue,
		NoErrorTruncation:   core.TSTrue,
	}
	host := compiler.NewCompilerHost(fs, bundled.LibPath(), nil, nil, nil)
	program := compiler.NewProgram(compiler.ProgramOptions{
		Config:         tsoptions.NewParsedCommandLine(options, names, nil, "/.src", tspath.CaseSensitive),
		Host:           host,
		SingleThreaded: core.TSTrue,
	})
	c, done := program.GetTypeChecker(context.Background())
	return program, c, done
}

// apiStabilityExport returns the named export of a source file.
func apiStabilityExport(t *testing.T, c *checker.Checker, sf *ast.SourceFile, name string) *ast.Symbol {
	t.Helper()
	moduleSymbol := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
	for _, symbol := range c.GetExportsOfModule(moduleSymbol) {
		if symbol != nil && symbol.Name == name {
			return symbol
		}
	}
	t.Fatalf("missing export %s", name)
	return nil
}

func TestApiStabilityLeakRelatedDeclaration(t *testing.T) {
	t.Parallel()
	for _, tc := range []struct {
		name       string
		files      map[string]string
		file       string
		identifier string
	}{
		{
			name: "local type",
			files: map[string]string{"test.ts": `
/** @stability experimental */
interface Experimental { value: string }
export interface Public { value: Experimental }
`},
			file: "test.ts", identifier: "Experimental",
		},
		{
			name: "cross-file re-export",
			files: map[string]string{
				"dep.ts": `
/** @stability unstable */
export interface Unstable { value: string }
export interface Public { value: Unstable }
`,
				"test.ts": `export { Public } from "./dep.js"`,
			},
			file: "dep.ts", identifier: "Unstable",
		},
		{
			name: "tagged overload",
			files: map[string]string{"test.ts": `
interface Callable {
  /** @stability experimental */
  (value: string): string;
  (value: number): number;
}
export interface Public { call: Callable }
`},
			file: "test.ts", identifier: "(value: string): string;",
		},
	} {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			program, c, done := apiStabilityProgram(t, tc.files, true)
			defer done()
			sf := program.GetSourceFile("/.src/test.ts")
			var leaks []*ast.Diagnostic
			for _, diagnostic := range c.GetDiagnostics(context.Background(), sf) {
				if diagnostic.Code() == 377137 {
					leaks = append(leaks, diagnostic)
				}
			}
			if len(leaks) != 1 {
				t.Fatalf("expected one leak, got %d", len(leaks))
			}
			if leaks[0].File() != sf || tc.files["test.ts"][leaks[0].Pos():leaks[0].End()] != "Public" {
				t.Fatal("primary diagnostic must point to the Public export")
			}
			related := leaks[0].RelatedInformation()
			if len(related) != 1 {
				t.Fatalf("expected one related declaration, got %d", len(related))
			}
			if related[0].File() != program.GetSourceFile(tspath.RootedFilePath("/.src/"+tc.file)) {
				t.Fatal("related diagnostic points to the wrong source file")
			}
			if got := tc.files[tc.file][related[0].Pos():related[0].End()]; got != tc.identifier {
				t.Fatalf("related location = %q, want %q", got, tc.identifier)
			}
			if !strings.Contains(related[0].String(), "is declared") {
				t.Fatalf("unexpected related message: %s", related[0].String())
			}
		})
	}
}

// apiStabilityDiagnostics collects the checker diagnostics for one file
// plus the global diagnostics, excluding the rule's own reports so a control
// run can be compared exactly with an enabled run.
func apiStabilityDiagnostics(t *testing.T, c *checker.Checker, sf *ast.SourceFile) []string {
	t.Helper()
	var out []string
	add := func(diagnostic *ast.Diagnostic) {
		if diagnostic == nil || diagnostic.String() == "" {
			return
		}
		if strings.Contains(diagnostic.String(), "effect(apiStabilityLeak)") {
			return
		}
		out = append(out, fmt.Sprintf("%d:%s", diagnostic.Code(), diagnostic.String()))
	}
	for _, diagnostic := range c.GetDiagnostics(context.Background(), sf) {
		add(diagnostic)
	}
	for _, diagnostic := range c.GetGlobalDiagnostics() {
		out = append(out, fmt.Sprintf("global:%d:%s", diagnostic.Code(), diagnostic.String()))
	}
	sort.Strings(out)
	return out
}

// apiStabilityProgramDiagnostics collects the whole-program semantic and
// global diagnostics, excluding the rule's own reports.
func apiStabilityProgramDiagnostics(t *testing.T, program *compiler.Program) []string {
	t.Helper()
	var out []string
	add := func(diagnostic *ast.Diagnostic) {
		if diagnostic == nil || diagnostic.String() == "" {
			return
		}
		if strings.Contains(diagnostic.String(), "effect(apiStabilityLeak)") {
			return
		}
		out = append(out, fmt.Sprintf("%d:%s", diagnostic.Code(), diagnostic.String()))
	}
	for _, diagnostic := range program.GetSemanticDiagnostics(context.Background(), nil) {
		add(diagnostic)
	}
	for _, diagnostic := range program.GetGlobalDiagnostics(context.Background()) {
		add(diagnostic)
	}
	sort.Strings(out)
	return out
}

// TestApiStabilityLeakWholeSurfaceSafety pins findings 1-4: a complete
// structured surface is never materialized when any declaration it would
// resolve (own members, inherited members, index values, alias defaults, wide
// tuples) can expand a recursive alias. The comparison against a disabled
// control includes global diagnostics, so the rule cannot add TS2589, and the
// reverse comparison ensures it cannot lose an ordinary diagnostic either.
func TestApiStabilityLeakWholeSurfaceSafety(t *testing.T) {
	t.Parallel()

	t.Run("recursive index value", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "recursive index value",
			map[string]string{"test.ts": apiStabilityRecursivePrefix + `interface I<T> { [s: string]: Deep<T> }
export declare const x: I<string>
`}, nil, nil)
	})

	t.Run("inherited recursive index value", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "inherited recursive index value",
			map[string]string{"test.ts": apiStabilityRecursivePrefix + `interface Base<T> { [s: string]: Deep<T> }
interface I<T> extends Base<T> {}
export declare const x: I<string>
`}, nil, nil)
	})

	t.Run("anonymous recursive index return", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "anonymous recursive index return",
			map[string]string{"test.ts": apiStabilityRecursivePrefix + `declare function make<T>(): { [s: string]: Deep<T> }
export const x = make<string>()
`}, nil, nil)
	})

	t.Run("compound indexed callable return", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "compound indexed callable return",
			map[string]string{"test.ts": apiStabilityRecursivePrefix + `interface I<T> { v: Deep<T> }
declare function make<T>(): () => T["v" & keyof T]
export const x = make<I<string>>()
`}, nil, nil)
	})

	t.Run("compound indexed parameter", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "compound indexed parameter",
			map[string]string{"test.ts": apiStabilityRecursivePrefix + `interface I<T> { v: Deep<T> }
declare function make<T>(): (x: T["v" & keyof T]) => void
export const x = make<I<string>>()
`}, nil, nil)
	})

	t.Run("compound indexed nested return", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "compound indexed nested return",
			map[string]string{"test.ts": apiStabilityRecursivePrefix + `interface I<T> { v: Deep<T> }
declare function make<T>(): () => { value: T["v" & keyof T] }
export const x = make<I<string>>()
`}, nil, nil)
	})

	t.Run("compound indexed alias return", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "compound indexed alias return",
			map[string]string{"test.ts": apiStabilityRecursivePrefix + `interface I<T> { v: Deep<T> }
type Read<T> = T["v" & keyof T]
declare function make<T>(): () => Read<T>
export const x = make<I<string>>()
`}, nil, nil)
	})

	t.Run("wide tuple recursive member", func(t *testing.T) {
		t.Parallel()
		elements := strings.Repeat("number,", 4100)
		runApiStabilitySafetyCase(t, "wide tuple recursive member",
			map[string]string{"test.ts": apiStabilityRecursivePrefix + `interface I<T> {
  v: [` + elements + `Deep<T>]
}
export declare const x: I<string>
`}, nil, nil)
	})

	t.Run("skipped declaration alias default", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "skipped declaration alias default", map[string]string{
			"test.ts":  `export * as ns from "./dep.js"` + "\n",
			"dep.d.ts": apiStabilityRecursivePrefix + "type D<T = Deep<string>> = T\nexport interface X { v: D }\n",
		}, nil, nil)
	})

	t.Run("generic default references earlier parameter", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "generic default references earlier parameter",
			map[string]string{"test.ts": apiStabilityRecursivePrefix + `interface I<A, B = Deep<A>> { v: B }
export declare const x: I<string>
`}, nil, nil)
	})
}

// TestApiStabilityLeakCoverage pins findings 5-8: bare generic binders
// and safe inferred closures expose their concrete dependency; flattened
// compound aliases keep their declared provenance; nested class statics stay
// shallow.
func TestApiStabilityLeakCoverage(t *testing.T) {
	t.Parallel()

	prefix := "/** @stability experimental */\ninterface E { v: string }\n"

	t.Run("bare generic callable return leaky first", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "bare generic callable return leaky first",
			map[string]string{"test.ts": prefix + `declare function make<T>(): () => T
export const x = make<E>()
`}, []string{"`x` exposes `E`"}, nil)
	})

	t.Run("bare generic callable return clean first", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "bare generic callable return clean first",
			map[string]string{"test.ts": `declare function make<T>(): () => T
export const x = make<E>()
/** @stability experimental */
interface E { v: string }
`}, []string{"`x` exposes `E`"}, nil)
	})

	t.Run("inferred closure", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "inferred closure",
			map[string]string{"test.ts": prefix + `declare const e: E
function make<T>(t: T) { return () => ({ v: t }) }
export const x = make(e)
`}, []string{"`x` exposes `E`"}, nil)
	})

	t.Run("inferred direct closure", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "inferred direct closure",
			map[string]string{"test.ts": prefix + `declare const e: E
function make<T>(t: T) { return () => t }
export const x = make(e)
`}, []string{"`x` exposes `E`"}, nil)
	})

	t.Run("inferred nested closure", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "inferred nested closure",
			map[string]string{"test.ts": prefix + `declare const e: E
function make<T>(t: T) { return () => () => t }
export const x = make(e)
`}, []string{"`x` exposes `E`"}, nil)
	})

	t.Run("inferred property closure", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "inferred property closure",
			map[string]string{"test.ts": prefix + `declare const e: E
function make<T>(t: T) { return { f: () => t } }
export const x = make(e)
`}, []string{"`x` exposes `E`"}, nil)
	})

	t.Run("flattened union alias", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "flattened union alias",
			map[string]string{"test.ts": "/** @stability experimental */\ntype A = string | number\nexport declare const x: A | boolean\n"},
			[]string{"`x` exposes `A`"}, nil)
	})

	t.Run("flattened intersection alias", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "flattened intersection alias",
			map[string]string{"test.ts": "/** @stability experimental */\ntype A = { a: string } & { b: number }\nexport declare const x: A & { c: boolean }\n"},
			[]string{"`x` exposes `A`"}, nil)
	})

	t.Run("nested class static shallow", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "nested class static shallow",
			map[string]string{"test.ts": prefix + `class C { static value: E }
export interface X { ctor: typeof C }
`}, nil, []string{"`X` exposes `E`"})
	})

	t.Run("benign default references earlier parameter", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "benign default references earlier parameter",
			map[string]string{"test.ts": prefix + `interface I<A, B = A> { v: B }
export declare const x: I<E>
`}, []string{"`x` exposes `E`"}, nil)
	})

	t.Run("declared infer conditional alias", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "declared infer conditional alias",
			map[string]string{"test.ts": prefix + `export type First<T> = T extends [infer U, ...unknown[]] ? E : never
`}, []string{"`First` exposes `E`"}, nil)
	})

	t.Run("materialized infer conditional outcome", func(t *testing.T) {
		t.Parallel()
		runApiStabilitySafetyCase(t, "materialized infer conditional outcome",
			map[string]string{"test.ts": prefix + `export declare const x: string[] extends Array<infer U> ? E : never
`}, []string{"`x` exposes `E`"}, nil)
	})
}

// TestApiStabilityLeakBareBinderOrders exercises the bare generic binder
// return with actual `make<string>()` / `make<E>()` applications across clean
// and leaky export orders on one shared checker. Five repeated queries per
// order pin that a shared cache never lets the concrete `string` application
// contaminate the `E` application or the other way around.
func TestApiStabilityLeakBareBinderOrders(t *testing.T) {
	t.Parallel()

	for _, order := range []struct {
		name   string
		source string
	}{
		{
			name: "clean first",
			source: `/** @stability experimental */
interface E { v: string }
declare function make<T>(): () => T
export const clean = make<string>()
export const leaky = make<E>()
`,
		},
		{
			name: "leaky first",
			source: `/** @stability experimental */
interface E { v: string }
declare function make<T>(): () => T
export const leaky = make<E>()
export const clean = make<string>()
`,
		},
	} {
		t.Run(order.name, func(t *testing.T) {
			t.Parallel()
			program, c, done := apiStabilityProgram(t, map[string]string{"test.ts": order.source}, true)
			defer done()
			sf := program.GetSourceFile("/.src/test.ts")
			_ = c.GetDiagnostics(context.Background(), sf)
			tp := typeparser.NewTypeParser(program, c)
			for i := range 5 {
				clean := tp.ApiStabilityUsedBySymbol(apiStabilityExport(t, c, sf, "clean"))
				leaky := tp.ApiStabilityUsedBySymbol(apiStabilityExport(t, c, sf, "leaky"))
				if clean.Minimum != typeparser.ApiStabilityStable {
					t.Fatalf("query %d: clean minimum=%d, want stable", i, clean.Minimum)
				}
				if leaky.Minimum != typeparser.ApiStabilityExperimental {
					t.Fatalf("query %d: leaky minimum=%d, want experimental", i, leaky.Minimum)
				}
			}
			if diagnostics := apiStabilityDiagnostics(t, c, sf); len(diagnostics) != 0 {
				t.Fatalf("rule changed compiler diagnostics: %v", diagnostics)
			}
		})
	}
}

// TestApiStabilityLeakSubsequentCheckerExactness checks the checker with
// the rule enabled and disabled after the same read sequence: a namespace
// barrel checked before the file that uses the recursive dependency. Both the
// later diagnostics (the TS2589 the ordinary read produces) and the ordinary
// type of the use site must match the control exactly, so the pre-flight guard
// can neither poison a later type nor consume a later diagnostic.
func TestApiStabilityLeakSubsequentCheckerExactness(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name  string
		files map[string]string
	}{
		{
			name: "recursive index namespace barrel",
			files: map[string]string{
				"test.ts": `export * as ns from "./dep.js"`,
				"dep.d.ts": apiStabilityRecursivePrefix + `export interface Base<T> { [s: string]: Deep<T> }
export interface I<T> extends Base<T> {}
export declare const x: I<string>`,
				"use.ts": `import { x } from "./dep.js"
export const y = x.foo`,
			},
		},
		{
			name: "skipped declaration alias default",
			files: map[string]string{
				"test.ts": `export * as ns from "./dep.js"`,
				"dep.d.ts": apiStabilityRecursivePrefix + `type D<T = Deep<string>> = T
export interface X { v: D }
export declare const x: X`,
				"use.ts": `import { x } from "./dep.js"
export const y = x.v`,
			},
		},
		{
			name: "compound indexed callable return",
			files: map[string]string{
				"test.ts": `export * as ns from "./dep.js"`,
				"dep.d.ts": apiStabilityRecursivePrefix + `export interface I<T> { v: Deep<T> }
declare function make<T>(): () => T["v" & keyof T]
export declare const x: ReturnType<typeof make<I<string>>>`,
				"use.ts": `import { x } from "./dep.js"
export const y = x()`,
			},
		},
	}

	type outcome struct {
		diagnostics []string
		useType     string
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			results := map[bool]outcome{}
			programResults := map[bool][]string{}
			for _, enabled := range []bool{false, true} {
				program, c, done := apiStabilityProgram(t, test.files, enabled)
				testFile := program.GetSourceFile("/.src/test.ts")
				_ = c.GetDiagnostics(context.Background(), testFile)
				useFile := program.GetSourceFile("/.src/use.ts")
				diagnostics := apiStabilityDiagnostics(t, c, useFile)
				y := apiStabilityExport(t, c, useFile, "y")
				results[enabled] = outcome{diagnostics: diagnostics, useType: c.TypeToString(c.GetTypeOfSymbol(y))}
				done()

				program, _, done = apiStabilityProgram(t, test.files, enabled)
				programResults[enabled] = apiStabilityProgramDiagnostics(t, program)
				done()
			}
			if !slices.Equal(results[false].diagnostics, results[true].diagnostics) {
				t.Errorf("later diagnostics differ:\ndisabled=%v\nenabled=%v", results[false].diagnostics, results[true].diagnostics)
			}
			if results[false].useType != results[true].useType {
				t.Errorf("later type differs: disabled=%q enabled=%q", results[false].useType, results[true].useType)
			}
			if !slices.Equal(programResults[false], programResults[true]) {
				t.Errorf("whole-program diagnostics differ:\ndisabled=%v\nenabled=%v", programResults[false], programResults[true])
			}
		})
	}
}

// TestApiStabilityLeakInferredForwardingDiagnostics checks that
// forwarding an ordinary inferred function resolves its body only through the
// checker's native lazy inference, which attributes diagnostics to the
// declaring file's own expressions: a later check of the declaring file must
// still report the function's own diagnostics, exactly as the rule-disabled
// control does, and the whole-program diagnostic set must be identical too.
func TestApiStabilityLeakInferredForwardingDiagnostics(t *testing.T) {
	t.Parallel()

	for _, expression := range []string{"nope", "(1).missing", "(1)()"} {
		t.Run(expression, func(t *testing.T) {
			t.Parallel()
			files := map[string]string{
				"test.ts": `export { f } from "./dep.js"`,
				"dep.ts":  `export function f() { return ` + expression + ` }`,
			}
			results := map[bool][]string{}
			programResults := map[bool][]string{}
			for _, enabled := range []bool{false, true} {
				program, c, done := apiStabilityProgram(t, files, enabled)
				_ = c.GetDiagnostics(context.Background(), program.GetSourceFile("/.src/test.ts"))
				results[enabled] = apiStabilityDiagnostics(t, c, program.GetSourceFile("/.src/dep.ts"))
				done()

				program, _, done = apiStabilityProgram(t, files, enabled)
				programResults[enabled] = apiStabilityProgramDiagnostics(t, program)
				done()
			}
			if !slices.Equal(results[false], results[true]) {
				t.Errorf("later dependency diagnostics differ:\ndisabled=%v\nenabled=%v", results[false], results[true])
			}
			if !slices.Equal(programResults[false], programResults[true]) {
				t.Errorf("whole-program diagnostics differ:\ndisabled=%v\nenabled=%v", programResults[false], programResults[true])
			}
			// The forwarding probe is only meaningful when the control actually
			// reports something at the later check.
			if len(results[false]) == 0 {
				t.Fatalf("control reported no later dependency diagnostics for %q", expression)
			}
		})
	}
}

// TestApiStabilityLeakBlockedSurfaceNotCached checks that a surface whose
// recursive component the guard refuses is never reported as stable and that
// repeating the query does no checker work. It then resolves the ordinary
// checker read to prove the pre-flight guard left the checker untouched: the
// finite inherited index resolves to its exact `number` value (not the error
// `any` a poisoned checker would report) and the recursive index still reports
// its own ordinary TS2589, both matching a rule-disabled control.
func TestApiStabilityLeakBlockedSurfaceNotCached(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name      string
		source    string
		wantValue string
	}{
		{
			name: "finite inherited index",
			source: `type Step<T extends unknown[]> = T extends [unknown, ...infer Tail] ? Step<Tail> : number
export interface Base<T extends unknown[]> { [s: string]: Step<T> }
export interface I<T extends unknown[]> extends Base<T> {}
export declare const x: I<[0, 0, 0, 0, 0, 0, 0, 0, 0, 0]>
`,
			wantValue: "number",
		},
		{
			name: "recursive inherited index",
			source: apiStabilityRecursivePrefix + `export interface Base<T> { [s: string]: Deep<T> }
export interface I<T> extends Base<T> {}
export declare const x: I<string>
`,
			wantValue: "any",
		},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			files := map[string]string{"test.ts": test.source}
			program, c, done := apiStabilityProgram(t, files, false)
			defer done()
			sf := program.GetSourceFile("/.src/test.ts")
			x := apiStabilityExport(t, c, sf, "x")
			tp := typeparser.NewTypeParser(program, c)

			var firstDelta uint32
			var last typeparser.ApiStabilityUsed
			for i := range 5 {
				before := c.TotalInstantiationCount
				last = tp.ApiStabilityUsedBySymbol(x)
				delta := c.TotalInstantiationCount - before
				if i == 0 {
					firstDelta = delta
					continue
				}
				if delta != 0 {
					t.Errorf("query %d repeated %d instantiations, want 0", i, delta)
				}
			}
			if !last.Incomplete {
				t.Error("a blocked recursive surface must report incomplete, not stable")
			}
			if firstDelta > 64 {
				t.Errorf("first query instantiated %d types, want a bounded pre-flight", firstDelta)
			}
			// The ordinary read must be exact and must not have been poisoned by
			// the analysis: the finite application resolves to its exact
			// declared value without a global diagnostic.
			value := c.GetTypeOfSymbol(apiStabilityExport(t, c, sf, "x"))
			infos := c.GetIndexInfosOfType(value)
			if len(infos) == 0 {
				t.Fatalf("ordinary index resolution lost its index info: %v", c.GetGlobalDiagnostics())
			}
			if got := c.TypeToString(infos[0].ValueType()); got != test.wantValue {
				t.Errorf("ordinary index value=%q, want %q (globals=%v)", got, test.wantValue, c.GetGlobalDiagnostics())
			}
			if test.name == "finite inherited index" && len(c.GetGlobalDiagnostics()) != 0 {
				t.Errorf("ordinary finite read produced global diagnostics: %v", c.GetGlobalDiagnostics())
			}
		})
	}
}

// apiStabilityRecursiveAlias is the self-expanding alias the regression
// probes use. Any speculative resolution of an application of it trips the
// checker's instantiation depth guard, so a rule that approves it would add
// TS2589 to a program whose own checking does not produce one.
const apiStabilityRecursiveAlias = "type Deep<T> = T extends string ? Deep<T> : T\n"

// TestApiStabilityLeakRelationSafety pins that the conditional-branch
// relation is only asked after a relation-operand pre-flight has established
// that resolving the full projected operand surfaces is bounded. Every probe
// runs the rule-enabled and rule-disabled programs through the same checker
// sequence and compares the checked file diagnostics, the later use-site
// diagnostics and types, and the global diagnostics exactly, so the rule can
// neither add nor lose a compiler diagnostic nor change a later type.
func TestApiStabilityLeakRelationSafety(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name string
		dep  string
		expr string
	}{
		{
			name: "callable relation",
			dep: apiStabilityRecursiveAlias + `interface F<T> { (): Deep<T> }
type Select<T> = T extends (() => number) ? string : number
export interface X { v: Select<F<string>> }
export declare const x: X`,
			expr: "x.v",
		},
		{
			name: "property relation",
			dep: apiStabilityRecursiveAlias + `interface F<T> { v: Deep<T> }
type Select<T> = T extends { v: number } ? string : number
export interface X { v: Select<F<string>> }
export declare const x: X`,
			expr: "x.v",
		},
		{
			name: "array conditional substitution",
			dep: apiStabilityRecursiveAlias + `type Select<T> = T[] extends string[] ? Deep<string> : number
export interface X { v: Select<string> }
export declare const x: X`,
			expr: "x.v",
		},
		{
			name: "reference conditional substitution",
			dep: apiStabilityRecursiveAlias + `interface Box<T> { value: T }
type Select<T> = Box<T> extends Box<string> ? Deep<string> : number
export interface X { v: Select<string> }
export declare const x: X`,
			expr: "x.v",
		},
		{
			name: "never conditional",
			dep: apiStabilityRecursiveAlias + `type Select = never extends string ? Deep<string> : number
export interface X { v: Select }
export declare const x: X`,
			expr: "x.v",
		},
		{
			name: "ignored explicit alias argument",
			dep: apiStabilityRecursiveAlias + `type Ignore<T> = number
export interface X { v: Ignore<Deep<string>> }
export declare const x: X`,
			expr: "x.v",
		},
		{
			name: "inherited projection",
			dep: apiStabilityRecursiveAlias + `interface Base<T> { v: Deep<T> }
interface F<T> extends Base<T> {}
type Read<T> = T["v" & keyof T]
export interface X { v: Read<F<string>> }
export declare const x: X`,
			expr: "x.v",
		},
	}

	type outcome struct {
		pre     []string
		later   []string
		useType string
		globals int
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			files := map[string]string{
				"test.ts":  `export * as ns from "./dep.js"`,
				"dep.d.ts": test.dep,
				"use.ts": `import { x } from "./dep.js"
export const y = ` + test.expr,
			}
			results := map[bool]outcome{}
			programResults := map[bool][]string{}
			for _, enabled := range []bool{false, true} {
				program, c, done := apiStabilityProgram(t, files, enabled)
				testFile := program.GetSourceFile("/.src/test.ts")
				before := c.TotalInstantiationCount
				pre := apiStabilityDiagnostics(t, c, testFile)
				delta := c.TotalInstantiationCount - before
				depFile := program.GetSourceFile("/.src/dep.d.ts")
				if enabled {
					// While the dependency is still unchecked, the refused
					// recursive read reports an incomplete result; nothing
					// computed is persisted for a later analysis.
					refused := typeparser.NewTypeParser(program, c).ApiStabilityUsedBySymbol(apiStabilityExport(t, c, depFile, "X"))
					if !refused.Incomplete {
						t.Error("a refused recursive surface must report incomplete before the dependency is checked")
					}
				}
				useFile := program.GetSourceFile("/.src/use.ts")
				later := apiStabilityDiagnostics(t, c, useFile)
				y := apiStabilityExport(t, c, useFile, "y")
				results[enabled] = outcome{
					pre:     pre,
					later:   later,
					useType: c.TypeToString(c.GetTypeOfSymbol(y)),
					globals: len(c.GetGlobalDiagnostics()),
				}
				t.Logf("enabled=%v delta=%d pre=%v later=%v type=%s globals=%d",
					enabled, delta, pre, later, results[enabled].useType, results[enabled].globals)
				if enabled && delta > 256 {
					t.Errorf("rule instantiated %d types before blocking, want at most 256", delta)
				}
				done()

				program, _, done = apiStabilityProgram(t, files, enabled)
				programResults[enabled] = apiStabilityProgramDiagnostics(t, program)
				done()
			}
			if !reflect.DeepEqual(results[false], results[true]) {
				t.Errorf("rule changed checker behavior:\ndisabled=%+v\nenabled=%+v", results[false], results[true])
			}
			if !slices.Equal(programResults[false], programResults[true]) {
				t.Errorf("rule changed whole-program diagnostics:\ndisabled=%v\nenabled=%v", programResults[false], programResults[true])
			}
		})
	}
}

// TestApiStabilityLeakRefusedSurfaceStaysRetryable repeats the direct
// symbol query on one shared checker: a refused recursive surface always
// reports an incomplete result, and every repeated query after the first
// performs no checker instantiation, which proves the guard failed closed
// instead of settling a partial result.
func TestApiStabilityLeakRefusedSurfaceStaysRetryable(t *testing.T) {
	t.Parallel()

	cases := []struct{ name, dep string }{
		{"callable relation", apiStabilityRecursiveAlias + `interface F<T> { (): Deep<T> }
type Select<T> = T extends (() => number) ? string : number
export interface X { v: Select<F<string>> }
export declare const x: X`},
		{"inherited projection", apiStabilityRecursiveAlias + `interface Base<T> { v: Deep<T> }
interface F<T> extends Base<T> {}
type Read<T> = T["v" & keyof T]
export interface X { v: Read<F<string>> }
export declare const x: X`},
		{"ignored explicit alias argument", apiStabilityRecursiveAlias + `type Ignore<T> = number
export interface X { v: Ignore<Deep<string>> }
export declare const x: X`},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			files := map[string]string{"dep.d.ts": test.dep, "test.ts": `export * as ns from "./dep.js"`}
			program, c, done := apiStabilityProgram(t, files, false)
			defer done()
			_ = c.GetDiagnostics(context.Background(), program.GetSourceFile("/.src/test.ts"))
			symbol := apiStabilityExport(t, c, program.GetSourceFile("/.src/dep.d.ts"), "X")
			tp := typeparser.NewTypeParser(program, c)
			for i := range 5 {
				before := c.TotalInstantiationCount
				used := tp.ApiStabilityUsedBySymbol(symbol)
				delta := c.TotalInstantiationCount - before
				if !used.Incomplete {
					t.Fatalf("query %d reported a refused surface as stable", i)
				}
				if i > 0 && delta != 0 {
					t.Errorf("query %d repeated %d instantiations, want 0", i, delta)
				}
			}
		})
	}
}

// TestApiStabilityLeakExplicitStableForwarding pins that a forwarding
// `@stability stable` tag on a re-export is an explicit ceiling, distinct from
// an absent tag: it tightens the forwarded symbol's inherited stability even
// though stable is the default tier. Named, star and namespace re-exports are
// covered.
func TestApiStabilityLeakExplicitStableForwarding(t *testing.T) {
	t.Parallel()

	dep := `/** @stability experimental */
interface E { v: string }
/** @stability experimental */
export interface X { v: E }`
	forwards := []struct {
		label string
		want  string
		// absentLeak is true when an untagged re-export still reports: a
		// namespace alias inherits the untagged module's stable default, while
		// named and star re-exports inherit the forwarded symbol's own tag.
		absentLeak bool
		source     string
	}{
		{"named", "`X` exposes `E`", false, `export { X } from "./dep.js"`},
		{"star", "`X` exposes `E`", false, `export * from "./dep.js"`},
		{"namespace", "`ns` exposes `E`", true, `export * as ns from "./dep.js"`},
	}
	for _, forward := range forwards {
		for _, tag := range []string{"stable", "unstable", "experimental", ""} {
			name := forward.label + "/" + tag
			if tag == "" {
				name = forward.label + "/absent"
			}
			t.Run(name, func(t *testing.T) {
				t.Parallel()
				source := forward.source
				if tag != "" {
					source = "/** @stability " + tag + " */\n" + source
				}
				files := map[string]string{"dep.d.ts": dep, "test.ts": source}
				got := apiStabilityLifecycleDiagnostics(t, files, true, false)
				control := apiStabilityLifecycleDiagnostics(t, files, false, false)
				// The rule may only add its own reports; every compiler
				// diagnostic must be exactly the control's.
				for _, message := range got {
					if strings.Contains(message, "effect(apiStabilityLeak)") {
						continue
					}
					if !apiStabilityLifecycleContains(control, message) {
						t.Errorf("added compiler diagnostic %q\nrule=%v\ncontrol=%v", message, got, control)
					}
				}
				for _, message := range control {
					if !apiStabilityLifecycleContains(got, message) {
						t.Errorf("lost compiler diagnostic %q\nrule=%v\ncontrol=%v", message, got, control)
					}
				}
				switch tag {
				case "stable", "unstable":
					if !apiStabilityLifecycleContains(got, forward.want) {
						t.Errorf("explicit %s ceiling ignored: %v", tag, got)
					}
				case "experimental":
					if apiStabilityLifecycleContains(got, "exposes") {
						t.Errorf("unexpected leak for experimental ceiling: %v", got)
					}
				case "":
					if forward.absentLeak != apiStabilityLifecycleContains(got, "exposes") {
						t.Errorf("absent ceiling leak=%v, want %v: %v",
							apiStabilityLifecycleContains(got, "exposes"), forward.absentLeak, got)
					}
				}
			})
		}
	}
}

// TestApiStabilityLeakInferredReexportCoverage pins that an ordinary
// inferred re-export is reported in both the per-file and the whole-program
// lifecycle, while the dependency's own diagnostics stay exactly the control's.
// Inference is the checker's native lazy path: its diagnostics are attributed
// to the declaring file's expressions, so a later check of that file observes
// the same diagnostics and types.
func TestApiStabilityLeakInferredReexportCoverage(t *testing.T) {
	t.Parallel()

	prefix := "/** @stability experimental */\ninterface E { v: string }\ndeclare const e: E\n"
	deps := []struct{ name, dep string }{
		{"function", prefix + `export function f() { return e }`},
		{"generic closure", prefix + `function make<T>(t: T) { return () => ({ v: t }) }
export const f = make(e)`},
		{"direct closure", prefix + `function make<T>(t: T) { return () => t }
export const f = make(e)`},
	}
	barrels := []struct{ name, source, want string }{
		{"named", `export { f } from "./dep.js"`, "exposes `E`"},
		{"star", `export * from "./dep.js"`, "exposes `E`"},
		{"namespace", `export * as ns from "./dep.js"`, "exposes `E`"},
	}
	for _, barrel := range barrels {
		for _, dep := range deps {
			t.Run(barrel.name+"/"+dep.name, func(t *testing.T) {
				t.Parallel()
				files := map[string]string{"test.ts": barrel.source, "dep.ts": dep.dep}
				for _, wholeProgram := range []bool{false, true} {
					got := apiStabilityLifecycleDiagnostics(t, files, true, wholeProgram)
					control := apiStabilityLifecycleDiagnostics(t, files, false, wholeProgram)
					if !apiStabilityLifecycleContains(got, barrel.want) {
						t.Errorf("missing inferred leak whole=%v: %v", wholeProgram, got)
					}
					for _, message := range got {
						if strings.Contains(message, "effect(apiStabilityLeak)") {
							continue
						}
						if !apiStabilityLifecycleContains(control, message) {
							t.Errorf("added compiler diagnostic %q\nrule=%v\ncontrol=%v", message, got, control)
						}
					}
					for _, message := range control {
						if !apiStabilityLifecycleContains(got, message) {
							t.Errorf("lost compiler diagnostic %q\nrule=%v\ncontrol=%v", message, got, control)
						}
					}
				}
			})
		}
	}
}

// TestApiStabilityLeakInferredDiagnosticsStayExact pins that reading an
// inferred re-export through native inference never changes the dependency's
// own diagnostics, including the TS2589 a genuinely recursive initializer
// produces. The per-file request and the whole-program run are both compared
// against the rule-disabled control exactly.
func TestApiStabilityLeakInferredDiagnosticsStayExact(t *testing.T) {
	t.Parallel()

	cases := []struct{ name, dep string }{
		{"name error body", `export function f() { return nope }`},
		{"recursive body", apiStabilityRecursiveAlias + `declare const deep: Deep<string>
export function f() { return deep }`},
		{"recursive initializer", apiStabilityRecursiveAlias + `declare const deep: Deep<string>
export const f = { nested: { deep } }`},
		{"bad initializer", `export const f = nope`},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			files := map[string]string{"test.ts": `export { f } from "./dep.js"`, "dep.ts": test.dep}
			fileResults := map[bool][]string{}
			programResults := map[bool][]string{}
			for _, enabled := range []bool{false, true} {
				program, c, done := apiStabilityProgram(t, files, enabled)
				pre := apiStabilityDiagnostics(t, c, program.GetSourceFile("/.src/test.ts"))
				later := apiStabilityDiagnostics(t, c, program.GetSourceFile("/.src/dep.ts"))
				fileResults[enabled] = append(pre, later...)
				done()

				program, _, done = apiStabilityProgram(t, files, enabled)
				programResults[enabled] = apiStabilityProgramDiagnostics(t, program)
				done()
			}
			if !slices.Equal(fileResults[false], fileResults[true]) {
				t.Errorf("dependency diagnostics differ:\ndisabled=%v\nenabled=%v", fileResults[false], fileResults[true])
			}
			if !slices.Equal(programResults[false], programResults[true]) {
				t.Errorf("whole-program diagnostics differ:\ndisabled=%v\nenabled=%v", programResults[false], programResults[true])
			}
		})
	}
}

// TestApiStabilityLeakRelationOperands pins the relation
// preflight fixes. A named property reached through a nested callable parameter
// or return and an anonymous compound operand whose raw structure mentions a
// substituted parameter must be refused before the compiler relation is asked:
// enabling the rule adds no compiler diagnostic, loses none, and reports the
// refused reads as incomplete.
func TestApiStabilityLeakRelationOperands(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name string
		dep  string
		ts   bool
	}{
		{
			name: "callable return",
			dep: `interface G<T> { v: Deep<T> }
interface F<T> { (): G<T> }
type Select<T> = T extends (() => { v: number }) ? string : number
export interface X { v: Select<F<string>> }`,
		},
		{
			name: "callable parameter",
			dep: `interface G<T> { v: Deep<T> }
interface F<T> { (x: G<T>): void }
type Select<T> = T extends ((x: { v: number }) => void) ? string : number
export interface X { v: Select<F<string>> }`,
		},
		{
			name: "construct return",
			dep: `interface G<T> { v: Deep<T> }
interface F<T> { new (): G<T> }
type Select<T> = T extends (new () => { v: number }) ? string : number
export interface X { v: Select<F<string>> }`,
		},
		{
			name: "method return",
			dep: `interface G<T> { v: Deep<T> }
interface F<T> { m(): G<T> }
type Select<T> = T extends { m(): { v: number } } ? string : number
export interface X { v: Select<F<string>> }`,
		},
		{
			name: "inherited callable return",
			dep: `interface G<T> { v: Deep<T> }
interface B<T> { (): G<T> }
interface F<T> extends B<T> {}
type Select<T> = T extends (() => { v: number }) ? string : number
export interface X { v: Select<F<string>> }`,
		},
		{
			name: "anonymous object compound",
			dep: `type Select<T> = { v: T } extends { v: string } ? Deep<string> : number
export interface X { v: Select<string> }`,
		},
		{
			name: "anonymous function compound",
			dep: `type Select<T> = (() => T) extends (() => string) ? Deep<string> : number
export interface X { v: Select<string> }`,
		},
		{
			name: "anonymous method compound",
			dep: `type Select<T> = { m(): T } extends { m(): string } ? Deep<string> : number
export interface X { v: Select<string> }`,
		},
		{
			name: "mapped compound",
			dep: `type Select<T> = { [K in keyof T]: Deep<K> } extends { toString(): string } ? number : Deep<string>
export interface X { v: Select<string> }`,
		},
		{
			name: "generic callable constraint",
			dep: `interface G<T> { v: Deep<T> }
interface F<T> { <U extends G<T>>(x: U): void }
type Select<T> = T extends (<U extends { v: number }>(x: U) => void) ? Deep<string> : number
export interface X { v: Select<F<string>> }`,
		},
		{
			name: "generic callable default",
			dep: `interface F<T> { <U = Deep<T>>(): U }
type Select<T> = T extends (() => number) ? Deep<string> : number
export interface X { v: Select<F<string>> }`,
		},
		{
			name: "inferred class property relation",
			ts:   true,
			dep: `declare function make<T>(): Deep<T>
class F { v = make<string>() }
type Select<T> = T extends { v: number } ? string : number
export interface X { y: Select<F> }`,
		},
	}

	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			depName := "dep.d.ts"
			if test.ts {
				depName = "dep.ts"
			}
			files := map[string]string{
				depName:   apiStabilityRecursivePrefix + test.dep,
				"test.ts": `export * as ns from "./dep.js"`,
				"use.ts":  "import { X } from \"./dep.js\"\nexport declare const z: X",
			}
			fileResults := map[bool][]string{}
			programResults := map[bool][]string{}
			for _, enabled := range []bool{false, true} {
				program, c, done := apiStabilityProgram(t, files, enabled)
				testFile := program.GetSourceFile("/.src/test.ts")
				useFile := program.GetSourceFile("/.src/use.ts")
				depFile := program.GetSourceFile(tspath.RootedFilePath("/.src/" + depName))
				var collected []string
				collected = append(collected, apiStabilityDiagnostics(t, c, testFile)...)
				collected = append(collected, apiStabilityDiagnostics(t, c, useFile)...)
				if enabled {
					// The rule must report a refused surface as incomplete
					// while the dependency is still unchecked. Once the
					// checker itself materializes the inferred component below,
					// completion is sound and is not asserted against.
					refused := typeparser.NewTypeParser(program, c).ApiStabilityUsedBySymbol(apiStabilityExport(t, c, depFile, "X"))
					if !refused.Incomplete {
						t.Errorf("refused surface X was not reported incomplete before the dependency was checked")
					}
				}
				// Check the dependency twice: a refused read must not gain or
				// lose a diagnostic between the first and the repeated check.
				first := apiStabilityDiagnostics(t, c, depFile)
				second := apiStabilityDiagnostics(t, c, depFile)
				if !reflect.DeepEqual(first, second) {
					t.Errorf("repeated dependency diagnostics differ:\nfirst=%v\nsecond=%v", first, second)
				}
				collected = append(collected, first...)
				fileResults[enabled] = collected
				done()

				program, _, done = apiStabilityProgram(t, files, enabled)
				programResults[enabled] = apiStabilityProgramDiagnostics(t, program)
				done()
			}
			if !reflect.DeepEqual(fileResults[false], fileResults[true]) {
				t.Errorf("rule changed checker behavior:\ndisabled=%v\nenabled=%v", fileResults[false], fileResults[true])
			}
			if !reflect.DeepEqual(programResults[false], programResults[true]) {
				t.Errorf("rule changed whole-program diagnostics:\ndisabled=%v\nenabled=%v", programResults[false], programResults[true])
			}
		})
	}
}

// TestApiStabilityLeakAccessors pins that inferred accessors re-exported
// through named, star and namespace barrels are reported in the per-file and
// whole-program lifecycles, including class accessors and accessors inside
// generic closures. Accessor body diagnostics and recursive inference must stay
// exactly the control's.
func TestApiStabilityLeakAccessors(t *testing.T) {
	t.Parallel()

	const prefix = "/** @stability experimental */\ninterface E { v: string }\ndeclare const e: E\n"
	cases := []struct {
		name    string
		dep     string
		barrel  string
		want    []string
		notWant []string
	}{
		{
			name:   "object accessor",
			dep:    prefix + `export const f = { get v() { return e } }`,
			barrel: `export { f } from "./dep.js"`,
			want:   []string{"`f` exposes `E`"},
		},
		{
			name:   "star forwarded object accessor",
			dep:    prefix + `export const f = { get v() { return e } }`,
			barrel: `export * from "./dep.js"`,
			want:   []string{"`f` exposes `E`"},
		},
		{
			name:   "namespace forwarded object accessor",
			dep:    prefix + `export const f = { get v() { return e } }`,
			barrel: `export * as ns from "./dep.js"`,
			want:   []string{"`ns` exposes `E`"},
		},
		{
			name:   "class accessor",
			dep:    prefix + `export class C { get value() { return e } }`,
			barrel: `export { C } from "./dep.js"`,
			want:   []string{"`C` exposes `E`"},
		},
		{
			name:   "generic closure accessor",
			dep:    prefix + "function make<T>(t: T) { return { get v() { return t } } }\nexport const f = make(e)",
			barrel: `export { f } from "./dep.js"`,
			want:   []string{"`f` exposes `E`"},
		},
		{
			name:   "setter parameter accessor",
			dep:    prefix + `export const f = { set v(x: E) {} }`,
			barrel: `export { f } from "./dep.js"`,
			want:   []string{"`f` exposes `E`"},
		},
		{
			name:    "accessor body name error",
			dep:     prefix + `export const f = { get v() { return nope } }`,
			barrel:  `export { f } from "./dep.js"`,
			notWant: []string{"exposes"},
		},
		{
			name:   "annotated accessor with error body",
			dep:    prefix + `export const f = { get v(): E { return nope } }`,
			barrel: `export { f } from "./dep.js"`,
			want:   []string{"`f` exposes `E`"},
		},
		{
			name:    "setter body error",
			dep:     prefix + `export const f = { set v(x: number) { x.missing } }`,
			barrel:  `export { f } from "./dep.js"`,
			notWant: []string{"exposes"},
		},
		{
			name:    "recursive inferred accessor",
			dep:     apiStabilityRecursivePrefix + "declare function make<T>(): Deep<T>\nexport const f = { get v() { return make<string>() } }",
			barrel:  `export { f } from "./dep.js"`,
			notWant: []string{"exposes"},
		},
		{
			name:    "recursive class accessor",
			dep:     apiStabilityRecursivePrefix + "declare const deep: Deep<string>\nexport class C { get value() { return deep } }",
			barrel:  `export { C } from "./dep.js"`,
			notWant: []string{"exposes"},
		},
	}
	for _, test := range cases {
		files := map[string]string{"test.ts": test.barrel, "dep.ts": test.dep}
		runApiStabilitySafetyCase(t, test.name, files, test.want, test.notWant)
	}
}

// apiStabilityAccessorDiagnostic serializes one diagnostic exactly so the enabled
// and disabled runs can be compared including positions, codes and messages.
func apiStabilityAccessorDiagnostic(diagnostic *ast.Diagnostic) string {
	name := "global"
	if diagnostic.File() != nil {
		name = string(diagnostic.File().FileName())
	}
	return name + ":" + diagnostic.String()
}

// TestApiStabilityLeakAccessorDiagnosticsExact forces the accessor
// through the rule lifecycle and compares the exact diagnostics of the barrel,
// the dependency file (checked twice) and the globals with a disabled control.
func TestApiStabilityLeakAccessorDiagnosticsExact(t *testing.T) {
	t.Parallel()

	const prefix = "/** @stability experimental */\ninterface E { v: string }\ndeclare const e: E\n"
	deps := []string{
		prefix + `export const f = { get v() { return e } }`,
		prefix + `export const f = { get v() { return nope } }`,
		prefix + `export const f = { get v() { const x: number = "s"; return x } }`,
		prefix + `export const f = { set v(x: number) { x.missing } }`,
		prefix + `export const f = { get v(): E { return nope } }`,
		prefix + `export class C { get value() { return nope } }`,
		prefix + "function make<T>(t: T) { return { get v() { return t } } }\nexport const f = make(e)",
	}
	for _, dep := range deps {
		t.Run(dep, func(t *testing.T) {
			t.Parallel()

			files := map[string]string{"test.ts": `export { f } from "./dep.js"`, "dep.ts": dep}
			out := map[bool][]string{}
			for _, enabled := range []bool{false, true} {
				program, c, done := apiStabilityProgram(t, files, enabled)
				var collected []string
				for _, file := range []string{"/.src/test.ts", "/.src/dep.ts", "/.src/dep.ts"} {
					for _, diagnostic := range c.GetDiagnostics(context.Background(), program.GetSourceFile(tspath.RootedFilePath(file))) {
						if strings.Contains(diagnostic.String(), "effect(apiStabilityLeak)") {
							continue
						}
						collected = append(collected, apiStabilityAccessorDiagnostic(diagnostic))
					}
					for _, diagnostic := range c.GetGlobalDiagnostics() {
						collected = append(collected, apiStabilityAccessorDiagnostic(diagnostic))
					}
				}
				out[enabled] = collected
				done()
			}
			if !reflect.DeepEqual(out[false], out[true]) {
				t.Errorf("accessor read changed diagnostics:\ndisabled=%v\nenabled=%v", strings.Join(out[false], "\n"), strings.Join(out[true], "\n"))
			}
		})
	}
}
