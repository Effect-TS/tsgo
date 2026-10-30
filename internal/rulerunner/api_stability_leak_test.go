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

func TestApiStabilityLeakLifecycleRecursiveSafety(t *testing.T) {
	t.Parallel()
	files := map[string]string{"test.ts": apiStabilityRecursivePrefix + `declare const key: Deep<string>
export interface X { [key]: number }
`}
	messages := apiStabilityLifecycleDiagnostics(t, files, true, true)
	control := apiStabilityLifecycleDiagnostics(t, files, false, true)
	if len(messages) != len(control) {
		t.Errorf("rule changed diagnostics for an already-erroring Deep input:\nrule=%v\ncontrol=%v", messages, control)
	}
	if len(control) == 0 {
		t.Fatal("expected the control to report the baseline TS2589")
	}
}

const apiStabilityRecursivePrefix = "type Deep<T> = T extends string ? Deep<T> : T\n"

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

func apiStabilityExport(t *testing.T, c *checker.Checker, sf *ast.SourceFile, name string) *ast.Symbol {
	t.Helper()
	moduleSymbol := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
	for _, symbol := range c.GetExportsOfModule(moduleSymbol) {
		if symbol != nil && ast.TSGetSymbolName(symbol) == name {
			return symbol
		}
	}
	t.Fatalf("missing export %s", name)
	return nil
}

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

func TestApiStabilityLeakWholeSurfaceSafety(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name    string
		files   map[string]string
		notWant []string
	}{
		{
			name: "recursive member",
			files: map[string]string{"test.ts": apiStabilityRecursivePrefix + `interface Wrapper<T> { value: Deep<T> }
export declare const exposed: Wrapper<string>
`},
		},
		{
			name: "recursive index value",
			files: map[string]string{"test.ts": apiStabilityRecursivePrefix + `interface I<T> { [s: string]: Deep<T> }
export declare const x: I<string>
`},
		},
		{
			name: "inherited recursive index value",
			files: map[string]string{"test.ts": apiStabilityRecursivePrefix + `interface Base<T> { [s: string]: Deep<T> }
interface I<T> extends Base<T> {}
export declare const x: I<string>
`},
		},
		{
			name: "anonymous recursive index return",
			files: map[string]string{"test.ts": apiStabilityRecursivePrefix + `declare function make<T>(): { [s: string]: Deep<T> }
export const x = make<string>()
`},
		},
		{
			name: "compound indexed callable return",
			files: map[string]string{"test.ts": apiStabilityRecursivePrefix + `interface I<T> { v: Deep<T> }
declare function make<T>(): () => T["v" & keyof T]
export const x = make<I<string>>()
`},
		},
		{
			name: "compound indexed parameter",
			files: map[string]string{"test.ts": apiStabilityRecursivePrefix + `interface I<T> { v: Deep<T> }
declare function make<T>(): (x: T["v" & keyof T]) => void
export const x = make<I<string>>()
`},
		},
		{
			name: "compound indexed nested return",
			files: map[string]string{"test.ts": apiStabilityRecursivePrefix + `interface I<T> { v: Deep<T> }
declare function make<T>(): () => { value: T["v" & keyof T] }
export const x = make<I<string>>()
`},
		},
		{
			name: "compound indexed alias return",
			files: map[string]string{"test.ts": apiStabilityRecursivePrefix + `interface I<T> { v: Deep<T> }
type Read<T> = T["v" & keyof T]
declare function make<T>(): () => Read<T>
export const x = make<I<string>>()
`},
		},
		{
			name: "wide tuple recursive member",
			files: map[string]string{"test.ts": apiStabilityRecursivePrefix + `interface I<T> {
  v: [` + strings.Repeat("number,", 4100) + `Deep<T>]
}
export declare const x: I<string>
`},
		},
		{
			name: "skipped declaration alias default",
			files: map[string]string{
				"test.ts":  `export * as ns from "./dep.js"` + "\n",
				"dep.d.ts": apiStabilityRecursivePrefix + "type D<T = Deep<string>> = T\nexport interface X { v: D }\n",
			},
		},
		{
			name: "generic default references earlier parameter",
			files: map[string]string{"test.ts": apiStabilityRecursivePrefix + `interface I<A, B = Deep<A>> { v: B }
export declare const x: I<string>
`},
		},
		{
			name: "internal export behind a star cycle",
			files: map[string]string{
				"test.ts": apiStabilityRecursivePrefix + "/** @internal */\nexport interface Cyclic { value: Deep<string> }\nexport * from \"./dep\"\n",
				"dep.ts":  "export * from \"./test\"\n",
			},
			notWant: []string{"`Cyclic` exposes"},
		},
		{
			name: "recursive inferred accessor",
			files: map[string]string{
				"test.ts": `export { f } from "./dep.js"`,
				"dep.ts":  apiStabilityRecursivePrefix + "declare function make<T>(): Deep<T>\nexport const f = { get v() { return make<string>() } }",
			},
			notWant: []string{"exposes"},
		},
		{
			name: "recursive class accessor",
			files: map[string]string{
				"test.ts": `export { C } from "./dep.js"`,
				"dep.ts":  apiStabilityRecursivePrefix + "declare const deep: Deep<string>\nexport class C { get value() { return deep } }",
			},
			notWant: []string{"exposes"},
		},
	}
	for _, test := range cases {
		runApiStabilitySafetyCase(t, test.name, test.files, nil, test.notWant)
	}
}

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
				if clean.Minimum() != typeparser.ApiStabilityStable {
					t.Fatalf("query %d: clean minimum=%d, want stable", i, clean.Minimum())
				}
				if leaky.Minimum() != typeparser.ApiStabilityExperimental {
					t.Fatalf("query %d: leaky minimum=%d, want experimental", i, leaky.Minimum())
				}
			}
			if diagnostics := apiStabilityDiagnostics(t, c, sf); len(diagnostics) != 0 {
				t.Fatalf("rule changed compiler diagnostics: %v", diagnostics)
			}
		})
	}
}

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
			if len(results[false]) == 0 {
				t.Fatalf("control reported no later dependency diagnostics for %q", expression)
			}
		})
	}
}

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
			if !last.Incomplete() {
				t.Error("a blocked recursive surface must report incomplete, not stable")
			}
			if firstDelta > 64 {
				t.Errorf("first query instantiated %d types, want a bounded pre-flight", firstDelta)
			}
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

func TestApiStabilityLeakRelationSafety(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name string
		dep  string
		expr string
	}{
		{
			name: "callable relation",
			dep: apiStabilityRecursivePrefix + `interface F<T> { (): Deep<T> }
type Select<T> = T extends (() => number) ? string : number
export interface X { v: Select<F<string>> }
export declare const x: X`,
			expr: "x.v",
		},
		{
			name: "property relation",
			dep: apiStabilityRecursivePrefix + `interface F<T> { v: Deep<T> }
type Select<T> = T extends { v: number } ? string : number
export interface X { v: Select<F<string>> }
export declare const x: X`,
			expr: "x.v",
		},
		{
			name: "array conditional substitution",
			dep: apiStabilityRecursivePrefix + `type Select<T> = T[] extends string[] ? Deep<string> : number
export interface X { v: Select<string> }
export declare const x: X`,
			expr: "x.v",
		},
		{
			name: "reference conditional substitution",
			dep: apiStabilityRecursivePrefix + `interface Box<T> { value: T }
type Select<T> = Box<T> extends Box<string> ? Deep<string> : number
export interface X { v: Select<string> }
export declare const x: X`,
			expr: "x.v",
		},
		{
			name: "never conditional",
			dep: apiStabilityRecursivePrefix + `type Select = never extends string ? Deep<string> : number
export interface X { v: Select }
export declare const x: X`,
			expr: "x.v",
		},
		{
			name: "ignored explicit alias argument",
			dep: apiStabilityRecursivePrefix + `type Ignore<T> = number
export interface X { v: Ignore<Deep<string>> }
export declare const x: X`,
			expr: "x.v",
		},
		{
			name: "inherited projection",
			dep: apiStabilityRecursivePrefix + `interface Base<T> { v: Deep<T> }
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
					refused := typeparser.NewTypeParser(program, c).ApiStabilityUsedBySymbol(apiStabilityExport(t, c, depFile, "X"))
					if !refused.Incomplete() {
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

func TestApiStabilityLeakRefusedSurfaceStaysRetryable(t *testing.T) {
	t.Parallel()

	cases := []struct{ name, dep string }{
		{"callable relation", apiStabilityRecursivePrefix + `interface F<T> { (): Deep<T> }
type Select<T> = T extends (() => number) ? string : number
export interface X { v: Select<F<string>> }
export declare const x: X`},
		{"inherited projection", apiStabilityRecursivePrefix + `interface Base<T> { v: Deep<T> }
interface F<T> extends Base<T> {}
type Read<T> = T["v" & keyof T]
export interface X { v: Read<F<string>> }
export declare const x: X`},
		{"ignored explicit alias argument", apiStabilityRecursivePrefix + `type Ignore<T> = number
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
				if !used.Incomplete() {
					t.Fatalf("query %d reported a refused surface as stable", i)
				}
				if i > 0 && delta != 0 {
					t.Errorf("query %d repeated %d instantiations, want 0", i, delta)
				}
			}
		})
	}
}

func TestApiStabilityLeakInferredDiagnosticsStayExact(t *testing.T) {
	t.Parallel()

	cases := []struct{ name, dep string }{
		{"name error body", `export function f() { return nope }`},
		{"recursive body", apiStabilityRecursivePrefix + `declare const deep: Deep<string>
export function f() { return deep }`},
		{"recursive initializer", apiStabilityRecursivePrefix + `declare const deep: Deep<string>
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
					refused := typeparser.NewTypeParser(program, c).ApiStabilityUsedBySymbol(apiStabilityExport(t, c, depFile, "X"))
					if !refused.Incomplete() {
						t.Errorf("refused surface X was not reported incomplete before the dependency was checked")
					}
				}
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

func apiStabilityAccessorDiagnostic(diagnostic *ast.Diagnostic) string {
	name := "global"
	if diagnostic.File() != nil {
		name = string(diagnostic.File().FileName())
	}
	return name + ":" + diagnostic.String()
}

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
