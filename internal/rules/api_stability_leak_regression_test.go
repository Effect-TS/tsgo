package rules

import (
	"context"
	"fmt"
	"slices"
	"strings"
	"testing"
	"testing/fstest"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/bundled"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

// compileApiStabilityFiles compiles a multi-file virtual project and runs the
// rule against test.ts. It mirrors compileApiStabilitySource but supports the
// dependency files needed to exercise namespace, barrel and star re-exports.
func compileApiStabilityFiles(t *testing.T, files map[string]string) (*rule.Context, func()) {
	t.Helper()
	return compileApiStabilityFilesPrimed(t, files, true)
}

// compileApiStabilityFilesPrimed optionally primes semantic diagnostics before
// handing back the checker. Primed checkers model the language service warm
// path; unprimed checkers model a cold first run where the rule itself may
// trigger checking. Both must be free of instantiation and added diagnostics.
func compileApiStabilityFilesPrimed(t *testing.T, files map[string]string, prime bool) (*rule.Context, func()) {
	t.Helper()

	testfs := map[string]any{}
	var fileNames []tspath.RootedFilePath
	for name, content := range files {
		path := "/.src/" + name
		testfs[path] = &fstest.MapFile{Data: []byte(content)}
		fileNames = append(fileNames, tspath.RootedFilePath(path))
	}
	slices.Sort(fileNames)

	fs := vfstest.FromMap(testfs, tspath.CaseSensitive)
	fs = bundled.WrapFS(fs)

	compilerOptions := &core.CompilerOptions{
		NewLine:             core.NewLineKindLF,
		SkipDefaultLibCheck: core.TSTrue,
		SkipLibCheck:        core.TSTrue,
		NoErrorTruncation:   core.TSTrue,
		Target:              core.ScriptTargetESNext,
		Module:              core.ModuleKindNodeNext,
		ModuleResolution:    core.ModuleResolutionKindNodeNext,
		Strict:              core.TSTrue,
	}
	host := compiler.NewCompilerHost(fs, bundled.LibPath(), nil, nil, nil)
	program := compiler.NewProgram(compiler.ProgramOptions{
		Config:         tsoptions.NewParsedCommandLine(compilerOptions, fileNames, nil, "/.src", tspath.CaseSensitive),
		Host:           host,
		SingleThreaded: core.TSTrue,
	})

	ctx := context.Background()
	if prime {
		_ = program.GetSemanticDiagnostics(ctx, nil)
	}
	c, done := program.GetTypeChecker(ctx)
	sf := program.GetSourceFile("/.src/test.ts")
	if sf == nil {
		done()
		t.Fatal("failed to get source file")
	}
	tp := typeparser.NewTypeParser(program, c)
	ruleCtx := rule.NewContext(ctx, program, c, tp, sf, nil, etscore.SeverityWarning)
	return ruleCtx, done
}

func runApiStabilityMessages(t *testing.T, ruleCtx *rule.Context) []string {
	t.Helper()
	diagnostics := ApiStabilityLeak.Run(ruleCtx)
	messages := make([]string, 0, len(diagnostics))
	for _, diagnostic := range diagnostics {
		messages = append(messages, diagnostic.String())
	}
	return messages
}

func containsApiStabilityMessage(t *testing.T, messages []string, want string) bool {
	t.Helper()
	return slices.ContainsFunc(messages, func(message string) bool {
		return strings.Contains(message, want)
	})
}

// apiStabilityCompilerDiagnostics snapshots the source-file and global
// diagnostics the checker currently reports, including codes such as TS2589
// ("Type instantiation is excessively deep").
func apiStabilityCompilerDiagnostics(ruleCtx *rule.Context) []string {
	var out []string
	for _, diagnostic := range ruleCtx.Checker.GetDiagnostics(context.Background(), ruleCtx.SourceFile) {
		out = append(out, fmt.Sprintf("%d:%s", diagnostic.Code(), diagnostic.String()))
	}
	for _, diagnostic := range ruleCtx.Checker.GetGlobalDiagnostics() {
		out = append(out, fmt.Sprintf("global:%d:%s", diagnostic.Code(), diagnostic.String()))
	}
	return out
}

// assertApiStabilitySourcesParse guards the regression fixtures themselves:
// broken type syntax would silently disable the coverage they are meant to
// provide.
func assertApiStabilitySourcesParse(t *testing.T, files map[string]string) {
	t.Helper()
	ruleCtx, done := compileApiStabilityFiles(t, files)
	defer done()
	program, ok := ruleCtx.Program.(*compiler.Program)
	if !ok {
		t.Fatalf("program is %T, want *compiler.Program", ruleCtx.Program)
	}
	for _, diagnostic := range program.GetSyntacticDiagnostics(context.Background(), nil) {
		t.Errorf("syntactic diagnostic %d: %s", diagnostic.Code(), diagnostic.String())
	}
}

type apiStabilityRegressionCase struct {
	name    string
	files   map[string]string
	want    []string
	notWant []string
}

// apiStabilityMaxInstantiationDelta bounds the checker instantiations the rule
// may trigger for one case. Ordinary public surfaces stay far below it; a
// recursive alias application reached through a lazy public read is refused by
// the pre-flight recursion guard before the checker touches it, so the work is
// bounded and must not scale with recursion depth. The non-scaling test
// TestApiStabilityLeakInstantiationDoesNotScaleWithDepth pins that property.
const apiStabilityMaxInstantiationDelta = 100_000

// apiStabilityControlCompilerDiagnostics compiles the same files without ever
// running the rule and snapshots the checker's diagnostics, so a genuinely cold
// path can be compared against a control that the rule never touched.
func apiStabilityControlCompilerDiagnostics(t *testing.T, files map[string]string) []string {
	t.Helper()
	ruleCtx, done := compileApiStabilityFilesPrimed(t, files, false)
	defer done()
	return apiStabilityCompilerDiagnostics(ruleCtx)
}

// runApiStabilityRegressionCase runs a case against both a warm (primed) and a
// genuinely cold (unprimed) checker.
//
// The warm checker models the normal rule lifecycle: the file has been type
// checked, so the public types the surface needs are materialized and every
// expected diagnostic must be reported. The cold checker models a first run
// before any checking: type-driven components are not materialized, so the
// analysis must return an incomplete result that is never reported as stable
// instead of resolving syntax or triggering recursive instantiation. A cold run
// therefore must produce no false positive and must leave the compiler's
// diagnostics unchanged; expecting a cold diagnostic is only valid for
// metadata-driven surfaces (declared `@stability` tags), which this harness
// still accepts
// through notWant only. Cold results that can be completed from metadata alone
// may legitimately report extra true diagnostics.
//
// The cold path snapshots diagnostics only after the rule ran and compares them
// with a separate control program that never ran the rule, so a TS2589 the rule
// provokes cannot be masked by pre-warming the checker.
func runApiStabilityRegressionCase(t *testing.T, test apiStabilityRegressionCase) {
	t.Helper()
	for _, cold := range []bool{false, true} {
		name := test.name
		if cold {
			name += "/cold"
		} else {
			name += "/warm"
		}
		t.Run(name, func(t *testing.T) {
			ruleCtx, done := compileApiStabilityFilesPrimed(t, test.files, !cold)
			beforeCompiler := []string(nil)
			if !cold {
				beforeCompiler = apiStabilityCompilerDiagnostics(ruleCtx)
			}
			beforeInstantiation := ruleCtx.Checker.TotalInstantiationCount
			messages := runApiStabilityMessages(t, ruleCtx)
			delta := ruleCtx.Checker.TotalInstantiationCount - beforeInstantiation
			afterCompiler := apiStabilityCompilerDiagnostics(ruleCtx)
			done()

			if delta > apiStabilityMaxInstantiationDelta {
				t.Errorf("rule instantiated %d types, want at most %d", delta, apiStabilityMaxInstantiationDelta)
			}
			if cold {
				if control := apiStabilityControlCompilerDiagnostics(t, test.files); !slices.Equal(control, afterCompiler) {
					t.Errorf("cold rule changed compiler diagnostics against control:\ncontrol=%v\nafter=%v", control, afterCompiler)
				}
			} else if !slices.Equal(beforeCompiler, afterCompiler) {
				t.Errorf("rule changed compiler diagnostics:\nbefore=%v\nafter=%v", beforeCompiler, afterCompiler)
			}
			if !cold {
				for _, want := range test.want {
					if !containsApiStabilityMessage(t, messages, want) {
						t.Errorf("missing diagnostic %q in %v", want, messages)
					}
				}
			}
			for _, notWant := range test.notWant {
				if containsApiStabilityMessage(t, messages, notWant) {
					t.Errorf("unexpected diagnostic %q in %v", notWant, messages)
				}
			}
		})
	}
}

const apiStabilityExperimentalPrefix = "/** @stability experimental */\ninterface E { value: string }\ndeclare const e: E\n"

func singleApiStabilityFile(source string) map[string]string {
	return map[string]string{"test.ts": apiStabilityExperimentalPrefix + source}
}

// TestApiStabilityLeakFindingRegressions pins the original findings:
// instantiation safety, inline surfaces, erased aliases, constraints/defaults,
// bounded traversal and accessibility.
func TestApiStabilityLeakFindingRegressions(t *testing.T) {
	t.Parallel()

	deepBoxes := strings.Repeat("Box<", 6) + "E" + strings.Repeat(">", 6)
	tests := []apiStabilityRegressionCase{
		{
			name: "generic budget is not exhausted",
			files: map[string]string{"test.ts": `type Deep<T> = T extends string ? Deep<T> : T
interface Wrapper<T> { value: Deep<T> }
export declare const exposed: Wrapper<string>
`},
			notWant: []string{"exposes"},
		},
		{
			name: "inline public object surfaces",
			files: map[string]string{"test.ts": `/** @stability experimental */
interface E { value: string }
export declare function parameter(x: { value: E }): void
export declare function result(): { value: E }
export interface Nested { nested: { value: E } }
export type Combined = { value: E } & { other: string }
`},
			want: []string{
				"`parameter` exposes `E`",
				"`result` exposes `E`",
				"`Nested` exposes `E`",
				"`Combined` exposes `E`",
			},
		},
		{
			name: "erased alias annotation",
			files: map[string]string{"test.ts": `/** @stability experimental */
type ExperimentalText = string
export declare function consume(x: ExperimentalText): ExperimentalText
`},
			want: []string{"`consume` exposes `ExperimentalText`"},
		},
		{
			name: "generic constraints and defaults",
			files: map[string]string{"test.ts": `/** @stability experimental */
interface E { value: string }
export declare function constrained<T extends E>(x: T): T
export declare function defaulted<T = E>(): T
export interface Generic<T extends E> { value: T }
export type Indexed<K extends keyof E> = E[K]
export type Conditional<T> = T extends E ? E : string
`},
			want: []string{
				"`constrained` exposes `E`",
				"`defaulted` exposes `E`",
				"`Generic` exposes `E`",
				"`Indexed` exposes `E`",
				"`Conditional` exposes `E`",
			},
		},
		{
			name: "finite traversal is not truncated",
			files: map[string]string{"test.ts": `/** @stability experimental */
interface E { value: string }
interface Box<T> { value: T }
export declare const deep: ` + deepBoxes + `
export interface OrderSensitive {
  deep: Box<Box<Box<Box<E>>>>
  direct: Box<E>
}
`},
			want: []string{
				"`deep` exposes `E`",
				"`OrderSensitive` exposes `E`",
			},
		},
		{
			name: "public private classification",
			files: map[string]string{"test.ts": `/** @stability experimental */
interface E { value: string }
export interface Public { __public: E }
export class Hidden { #secret: E | undefined }
export class Factory { private constructor(x: E) {} }
`},
			want:    []string{"`Public` exposes `E`"},
			notWant: []string{"`Hidden`", "`Factory`"},
		},
		{
			name: "callable surfaces",
			files: map[string]string{"test.ts": `/** @stability experimental */
interface E { value: string }
interface Callable { (x: E): void }
export interface Owner { value: Callable }
export interface Inline { value: (x: E) => void }
`},
			want: []string{"`Owner` exposes `E`", "`Inline` exposes `E`"},
		},
		{
			name: "namespace reexport",
			files: map[string]string{
				"test.ts": "export * as api from \"./dep\"\n",
				"dep.ts": `/** @stability experimental */
export interface E { value: string }
export interface Leaky { value: E }
`,
			},
			want: []string{"`api` exposes `E`"},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, test.files)
			runApiStabilityRegressionCase(t, test)
		})
	}
}

// TestApiStabilityLeakInferredAndReexportRegressions pins the inferred-surface
// findings: inferred accessor budget safety, inferred/arrow/callable
// signatures, inferred anonymous surfaces, inferred parameters, alias and
// namespace barrels, hidden overload implementations and reexport tags.
func TestApiStabilityLeakInferredAndReexportRegressions(t *testing.T) {
	t.Parallel()

	genericCallableBudget := `type Deep<T> = T extends string ? Deep<T> : T
interface Callable<T> { (x: Deep<T>): void }
declare const budgetValue: Callable<string>
export class Owner { get method() { return budgetValue } }
`
	genericCallableReturnBudget := `type Deep<T> = T extends string ? Deep<T> : T
interface Callable<T> { (): Deep<T> }
declare const budgetValue: Callable<string>
export class Owner { get method() { return budgetValue } }
`
	referencedGetterBudget := `type Deep<T> = T extends string ? Deep<T> : T
interface Callable<T> { (x: Deep<T>): void }
declare const budgetValue: Callable<string>
class Hidden { get method() { return budgetValue } }
export declare const owner: Hidden
`

	barrelDep := `/** @stability experimental */
interface E { value: string }
export interface Leaky { value: E }
`

	tests := []apiStabilityRegressionCase{
		// Finding 1: inferred accessors must not instantiate referenced generic signatures.
		{
			name:    "inferred getter budget",
			files:   map[string]string{"test.ts": genericCallableBudget},
			notWant: []string{"exposes"},
		},
		{
			name:    "inferred getter return budget",
			files:   map[string]string{"test.ts": genericCallableReturnBudget},
			notWant: []string{"exposes"},
		},
		{
			name:    "referenced non-generic getter budget",
			files:   map[string]string{"test.ts": referencedGetterBudget},
			notWant: []string{"exposes"},
		},

		// Finding 2: exported callable values keep their public signatures.
		{name: "arrow explicit", files: singleApiStabilityFile(`export const arrow = (x: E): E => x`), want: []string{"`arrow` exposes `E`"}},
		{name: "default arrow", files: singleApiStabilityFile(`export default (x: E): E => x`), want: []string{"`default` exposes `E`"}},
		{name: "function expression", files: singleApiStabilityFile(`export const f = function (x: E): E { return x }`), want: []string{"`f` exposes `E`"}},
		{
			name:  "typeof generic function",
			files: singleApiStabilityFile(`declare function generic<T>(x: E): T` + "\n" + `export type F = typeof generic`),
			want:  []string{"`F` exposes `E`"},
		},
		{
			name:  "generic callable direct export",
			files: singleApiStabilityFile("interface Callable<T> { (x: E): T }\nexport declare const callable: Callable<string>"),
			want:  []string{"`callable` exposes `E`"},
		},
		{
			name:  "inferred callable reference",
			files: singleApiStabilityFile("interface Callable { (x: E): E }\ndeclare const callable: Callable\nexport const f = callable"),
			want:  []string{"`f` exposes `E`"},
		},

		// Finding 3: direct inferred object surfaces and accessor values.
		{name: "inferred object", files: singleApiStabilityFile(`export const object = { value: e }`), want: []string{"`object` exposes `E`"}},
		{name: "inferred object nested", files: singleApiStabilityFile(`export const object = { nested: { value: e } }`), want: []string{"`object` exposes `E`"}},
		{name: "inferred function object", files: singleApiStabilityFile(`export function result() { return { value: e } }`), want: []string{"`result` exposes `E`"}},
		{name: "inferred function nested object", files: singleApiStabilityFile(`export function result() { return { nested: { value: e } } }`), want: []string{"`result` exposes `E`"}},
		{name: "inferred arrow nested object", files: singleApiStabilityFile(`export const result = () => ({ nested: { value: e } })`), want: []string{"`result` exposes `E`"}},
		{name: "inferred class property object", files: singleApiStabilityFile(`export class C { property = { value: e } }`), want: []string{"`C` exposes `E`"}},
		{name: "inferred getter", files: singleApiStabilityFile(`export class C { get value() { return e } }`), want: []string{"`C` exposes `E`"}},
		{name: "inferred class arrow property", files: singleApiStabilityFile(`export class C { value = (x: E): E => x }`), want: []string{"`C` exposes `E`"}},
		{name: "class expression", files: singleApiStabilityFile(`export const C = class { value: E = e; constructor(x: E) {} }`), want: []string{"`C` exposes `E`"}},
		{
			name:  "typeof inferred object",
			files: singleApiStabilityFile("const inferred = { value: e }\nexport type F = typeof inferred"),
			want:  []string{"`F` exposes `E`"},
		},
		{
			name:  "typeof inferred arrow",
			files: singleApiStabilityFile("const inferred = (x: E): E => x\nexport type F = typeof inferred"),
			want:  []string{"`F` exposes `E`"},
		},

		// Finding 4: inferred parameters, including constructors.
		{name: "parameter initializer", files: singleApiStabilityFile(`export function consume(x = e): void {}`), want: []string{"`consume` exposes `E`"}},
		{name: "constructor parameter initializer", files: singleApiStabilityFile(`export class Factory { constructor(x = e) {} }`), want: []string{"`Factory` exposes `E`"}},
		{
			name:  "inferred parameter object",
			files: singleApiStabilityFile(`export function consume(x = { value: e }): void {}`),
			want:  []string{"`consume` exposes `E`"},
		},

		// Finding 5: alias surfaces and namespace barrels.
		{
			name: "namespace barrel aliased member",
			files: map[string]string{
				"test.ts":   `export * as api from "./barrel"`,
				"barrel.ts": `export { Leaky } from "./dep"`,
				"dep.ts":    barrelDep,
			},
			want: []string{"`api` exposes `E`"},
		},
		{
			name: "namespace nested reexport",
			files: map[string]string{
				"test.ts":   `export * as api from "./barrel"`,
				"barrel.ts": `export * as nested from "./dep"`,
				"dep.ts":    barrelDep,
			},
			want: []string{"`api` exposes `E`"},
		},
		{
			name: "imported callable",
			files: map[string]string{
				"test.ts": `import type { Callable } from "./dep"
export interface O { f: Callable }
`,
				"dep.ts": `/** @stability experimental */
interface E { value: string }
export interface Callable { (x: E): E }
`,
			},
			want: []string{"`O` exposes `E`"},
		},
		{
			name: "imported alias argument",
			files: map[string]string{
				"test.ts": `import type { Holder } from "./dep"
export declare const x: Holder
`,
				"dep.ts": `/** @stability experimental */
interface E { value: string }
interface Box<T> { value: T }
export type Holder = Box<E>
`,
			},
			want: []string{"`x` exposes `E`"},
		},
		{
			name: "namespace cycle terminates",
			files: map[string]string{
				"test.ts": `export * as api from "./barrel"`,
				"barrel.ts": `export * as back from "./test"
/** @stability experimental */
interface E { value: string }
export interface Leaky { value: E }
`,
			},
			want: []string{"`api` exposes `E`"},
		},
		{
			name: "namespace alias cycle terminates",
			files: map[string]string{
				"test.ts": `export { x } from "./dep"`,
				"dep.ts":  `export { x } from "./test"`,
			},
			notWant: []string{"exposes"},
		},

		// Finding 6: hidden overload implementations do not leak.
		{
			name: "overload implementation hidden",
			files: singleApiStabilityFile(`export function f(x: string): string
export function f(x: string | E): string { return "" }
`),
			notWant: []string{"exposes"},
		},
		{
			name: "overload public signature leaks",
			files: singleApiStabilityFile(`export function f(x: E): E
export function f(x: string): string
export function f(x: E | string): E | string { return x as E }
`),
			want: []string{"`f` exposes `E`"},
		},

		// Finding 7: tags on reexport declarations govern their ceiling.
		{
			name: "named reexport experimental ceiling",
			files: map[string]string{
				"test.ts": `/** @stability experimental */
export { Leaky } from "./dep"
`,
				"dep.ts": barrelDep,
			},
			notWant: []string{"exposes"},
		},
		{
			name: "named reexport warns without tag",
			files: map[string]string{
				"test.ts": `export { Leaky } from "./dep"`,
				"dep.ts":  barrelDep,
			},
			want: []string{"`Leaky` exposes `E`"},
		},
		{
			name: "namespace unstable ceiling",
			files: map[string]string{
				"test.ts": `/** @stability unstable */
export * as api from "./dep"
`,
				"dep.ts": `/** @stability unstable */
export interface U { value: string }
export interface Leaky { value: U }
`,
			},
			notWant: []string{"exposes"},
		},
		{
			name: "namespace experimental ceiling",
			files: map[string]string{
				"test.ts": `/** @stability experimental */
export * as api from "./dep"
`,
				"dep.ts": `/** @stability experimental */
export interface E { value: string }
export interface Leaky { value: E }
`,
			},
			notWant: []string{"exposes"},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, test.files)
			runApiStabilityRegressionCase(t, test)
		})
	}
}

// TestApiStabilityLeakInstantiationAndSignatureRegressions pins instantiated
// anonymous budget safety, signature substitutions,
// instantiation expressions, public generic callable surfaces inside members and
// unions, eliminated conditional branches, erased accessor aliases, namespace
// member stability and star re-export tags.
func TestApiStabilityLeakInstantiationAndSignatureRegressions(t *testing.T) {
	t.Parallel()

	experimentalE := `/** @stability experimental */
interface E { value: string }
`

	tests := []apiStabilityRegressionCase{
		{
			// P1: an anonymous object instantiated from a generic function may
			// contain a lazily instantiated recursive member. Reading its members
			// through the uninstantiated target plus the mapper must not evaluate
			// `Deep<string>`.
			name: "instantiated anonymous object budget",
			files: map[string]string{"test.ts": `type Deep<T> = T extends string ? Deep<T> : T
declare function make<T>(): { value: Deep<T> }
export const object = make<string>()
`},
			notWant: []string{"exposes"},
		},
		{
			name: "inferred generic return substitution",
			files: map[string]string{"test.ts": experimentalE + `declare function make<T>(): (x: T) => T
export const f = make<E>()
`},
			want: []string{"`f` exposes `E`"},
		},
		{
			name: "instantiation expression signature",
			files: map[string]string{"test.ts": experimentalE + `declare function identity<T>(x: T): T
export const f = identity<E>
`},
			want: []string{"`f` exposes `E`"},
		},
		{
			name: "replaced generic default is not reported",
			files: map[string]string{"test.ts": experimentalE + `interface Callable<T = E> { (x: T): T }
export declare const f: Callable<string>
`},
			notWant: []string{"`f` exposes `E`"},
		},
		{
			name: "generic callable argument in member and union",
			files: map[string]string{"test.ts": experimentalE + `interface Callable<T> { (x: E): T }
export interface O { f: Callable<string> }
export declare const f: Callable<string> | undefined
`},
			want: []string{"`O` exposes `E`", "`f` exposes `E`"},
		},
		{
			name: "eliminated conditional branch is not reported",
			files: map[string]string{"test.ts": experimentalE + `export type Reduced = string extends string ? number : E
`},
			notWant: []string{"`Reduced` exposes `E`"},
		},
		{
			name: "erased accessor alias",
			files: map[string]string{"test.ts": `/** @stability experimental */
type ExperimentalText = string
export class C {
  get value(): ExperimentalText { return "x" }
}
`},
			want: []string{"`C` exposes `ExperimentalText`"},
		},
		{
			name: "namespace member own stability",
			files: map[string]string{
				"test.ts": `export * as api from "./dep"
`,
				"dep.ts": `/** @stability experimental */
export interface E { value: string }
`,
			},
			want: []string{"`api` exposes `E`"},
		},
		{
			name: "star reexport tag governs ceiling",
			files: map[string]string{
				"test.ts": `/** @stability experimental */
export * from "./dep"
`,
				"dep.ts": `/** @stability experimental */
interface E { value: string }
export interface Leaky { value: E }
`,
			},
			notWant: []string{"exposes"},
		},
		{
			name: "star unstable reexport does not report experimental",
			files: map[string]string{
				"test.ts": `/** @stability unstable */
export * from "./dep"
`,
				"dep.ts": `/** @stability experimental */
export interface E { value: string }
export interface Leaky { value: E }
`,
			},
			want: []string{"`Leaky` exposes `E`"},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, test.files)
			runApiStabilityRegressionCase(t, test)
		})
	}
}

// TestApiStabilityLeakAnonymousInstantiationColdBudget pins the cold-budget P1
// blocker on a genuinely unprimed checker: the rule runs before any diagnostic
// snapshot, and the resulting diagnostics are compared with a separate control
// program that never ran the rule, so a TS2589 produced by the rule itself
// cannot be masked by pre-warming.
func TestApiStabilityLeakAnonymousInstantiationColdBudget(t *testing.T) {
	t.Parallel()

	files := map[string]string{"test.ts": `type Deep<T> = T extends string ? Deep<T> : T
declare function make<T>(): { value: Deep<T> }
export const object = make<string>()
`}

	for _, cold := range []bool{false, true} {
		t.Run(fmt.Sprintf("cold=%v", cold), func(t *testing.T) {
			t.Parallel()
			ruleCtx, done := compileApiStabilityFilesPrimed(t, files, !cold)
			beforeCompiler := []string(nil)
			if !cold {
				beforeCompiler = apiStabilityCompilerDiagnostics(ruleCtx)
			}
			beforeInstantiation := ruleCtx.Checker.TotalInstantiationCount
			messages := runApiStabilityMessages(t, ruleCtx)
			delta := ruleCtx.Checker.TotalInstantiationCount - beforeInstantiation
			compiler := apiStabilityCompilerDiagnostics(ruleCtx)
			done()

			if delta > apiStabilityMaxInstantiationDelta {
				t.Errorf("rule instantiated %d types, want at most %d", delta, apiStabilityMaxInstantiationDelta)
			}
			if cold {
				control := apiStabilityControlCompilerDiagnostics(t, files)
				if !slices.Equal(control, compiler) {
					t.Errorf("cold rule changed compiler diagnostics against control:\ncontrol=%v\nafter=%v", control, compiler)
				}
			} else if !slices.Equal(beforeCompiler, compiler) {
				t.Errorf("rule changed compiler diagnostics:\nbefore=%v\nafter=%v", beforeCompiler, compiler)
			}
			if len(messages) != 0 {
				t.Errorf("got %d diagnostics, want 0: %v", len(messages), messages)
			}
		})
	}
}

// TestApiStabilityLeakInspectionIsCycleSafe guards against unbounded recursion
// when a public surface refers back to itself through non-generic references.
func TestApiStabilityLeakInspectionIsCycleSafe(t *testing.T) {
	t.Parallel()

	ruleCtx, done := compileApiStabilityFiles(t, map[string]string{"test.ts": `/** @stability experimental */
interface E { value: string }
export interface Node { next: Node; payload: E }
`})
	defer done()

	before := ruleCtx.Checker.TotalInstantiationCount
	messages := runApiStabilityMessages(t, ruleCtx)
	after := ruleCtx.Checker.TotalInstantiationCount

	if after != before {
		t.Errorf("rule instantiated %d types, want 0", after-before)
	}
	if !containsApiStabilityMessage(t, messages, "`Node` exposes `E`") {
		t.Fatalf("missing self-referential leak diagnostic: %v", messages)
	}
}

// TestApiStabilityLeakDoesNotTruncateMembers covers the previous first-128
// member cutoff: a leak positioned after many harmless members is still found.
func TestApiStabilityLeakDoesNotTruncateMembers(t *testing.T) {
	t.Parallel()

	var source strings.Builder
	source.WriteString("/** @stability experimental */\ninterface E { value: string }\n")
	source.WriteString("export interface ManyMembers {\n")
	for index := range 129 {
		fmt.Fprintf(&source, "  harmless%d: string\n", index)
	}
	source.WriteString("  leaked: E\n}\n")

	ruleCtx, done := compileApiStabilityFiles(t, map[string]string{"test.ts": source.String()})
	defer done()

	messages := runApiStabilityMessages(t, ruleCtx)
	if !containsApiStabilityMessage(t, messages, "`ManyMembers` exposes `E`") {
		t.Fatalf("leak after many members was not reported: %v", messages)
	}
}

// TestApiStabilityLeakLongAliasChain covers a named re-export chain longer than
// the old 32-hop resolution cap. Alias resolution is cycle-safe by identity, so
// a 40-hop chain still forwards the public surface.
func TestApiStabilityLeakLongAliasChain(t *testing.T) {
	t.Parallel()

	files := map[string]string{
		"test.ts": `export { Leaky } from "./barrel0"`,
		"dep.ts": `/** @stability experimental */
interface E { value: string }
export interface Leaky { value: E }
`,
	}
	for index := range 40 {
		next := fmt.Sprintf("barrel%d", index+1)
		if index == 39 {
			next = "dep"
		}
		files[fmt.Sprintf("barrel%d.ts", index)] = fmt.Sprintf("export { Leaky } from %q", "./"+next)
	}

	assertApiStabilitySourcesParse(t, files)
	ruleCtx, done := compileApiStabilityFiles(t, files)
	defer done()

	before := ruleCtx.Checker.TotalInstantiationCount
	messages := runApiStabilityMessages(t, ruleCtx)
	delta := ruleCtx.Checker.TotalInstantiationCount - before
	if delta != 0 {
		t.Errorf("rule instantiated %d types, want 0", delta)
	}
	if !containsApiStabilityMessage(t, messages, "`Leaky` exposes `E`") {
		t.Fatalf("40-hop alias chain lost its public surface: %v", messages)
	}
}

// TestApiStabilityLeakBudgetDiagnostics reproduces the budget blocker
// directly: running the rule on the inferred accessor must neither add TS2589
// (source or global) nor scale compiler instantiations, on warm and genuinely
// cold checkers. The cold diagnostics are compared with a separate control.
func TestApiStabilityLeakBudgetDiagnostics(t *testing.T) {
	t.Parallel()

	files := map[string]string{"test.ts": `type Deep<T> = T extends string ? Deep<T> : T
interface Callable<T> { (x: Deep<T>): void }
declare const c: Callable<string>
export class Owner { get method() { return c } }
`}
	for _, cold := range []bool{false, true} {
		t.Run(fmt.Sprintf("cold=%v", cold), func(t *testing.T) {
			t.Parallel()
			ruleCtx, done := compileApiStabilityFilesPrimed(t, files, !cold)
			beforeCompiler := []string(nil)
			if !cold {
				beforeCompiler = apiStabilityCompilerDiagnostics(ruleCtx)
			}
			beforeInstantiation := ruleCtx.Checker.TotalInstantiationCount
			messages := runApiStabilityMessages(t, ruleCtx)
			delta := ruleCtx.Checker.TotalInstantiationCount - beforeInstantiation
			afterCompiler := apiStabilityCompilerDiagnostics(ruleCtx)
			done()

			if delta > apiStabilityMaxInstantiationDelta {
				t.Errorf("rule instantiated %d types, want at most %d", delta, apiStabilityMaxInstantiationDelta)
			}
			if cold {
				control := apiStabilityControlCompilerDiagnostics(t, files)
				if !slices.Equal(control, afterCompiler) {
					t.Errorf("cold rule changed compiler diagnostics against control:\ncontrol=%v\nafter=%v", control, afterCompiler)
				}
			} else if !slices.Equal(beforeCompiler, afterCompiler) {
				t.Errorf("rule added compiler diagnostics:\nbefore=%v\nafter=%v", beforeCompiler, afterCompiler)
			}
			if len(messages) != 0 {
				t.Errorf("got %d diagnostics, want 0: %v", len(messages), messages)
			}
		})
	}
}

// TestApiStabilityLeakInheritedGenericAndCacheRegressions pins inherited generic
// surface budget safety, substitution-context cache
// separation, sound cycle completion, nested named shallow surfaces, declared
// member/signature stability, replaced defaults, generic conditional branches,
// forwarding ceiling precedence and erased aliases inside unions and reference
// arguments. Order-sensitive cases run in both declaration orders and every
// case runs on warm and genuinely cold checkers.
func TestApiStabilityLeakInheritedGenericAndCacheRegressions(t *testing.T) {
	t.Parallel()

	experimentalE := `/** @stability experimental */
interface E { value: string }
`
	deepAlias := `type Deep<T> = T extends string ? Deep<T> : T
`

	tests := []apiStabilityRegressionCase{
		{
			// P1: an inherited member of an uninstantiated interface can carry a
			// concrete recursive alias. It must be read from the base declaration
			// with a substitution, never by resolving the instantiated surface.
			name: "inherited generic member budget",
			files: map[string]string{"test.ts": deepAlias + `interface Base<T> { value: Deep<T> }
export interface Child extends Base<string> {}
`},
			notWant: []string{"exposes"},
		},
		{
			name: "inherited generic signature budget",
			files: map[string]string{"test.ts": deepAlias + `interface Base<T> { (x: T): void }
interface Child<T> extends Base<Deep<T>> {}
declare function make<T>(): Child<T>
export const f = make<string>()
`},
			notWant: []string{"exposes"},
		},
		{
			// P2: the same raw signature analyzed under different enclosing
			// substitutions must not share a cached result.
			name: "instantiated signature contexts clean first",
			files: map[string]string{"test.ts": experimentalE + `declare function make<T>(): (x: T) => T
export const clean = make<string>()
export const leaky = make<E>()
`},
			want:    []string{"`leaky` exposes `E`"},
			notWant: []string{"`clean` exposes `E`"},
		},
		{
			name: "instantiated signature contexts leaky first",
			files: map[string]string{"test.ts": experimentalE + `declare function make<T>(): (x: T) => T
export const leaky = make<E>()
export const clean = make<string>()
`},
			want:    []string{"`leaky` exposes `E`"},
			notWant: []string{"`clean` exposes `E`"},
		},
		{
			name: "nested instantiated contexts clean first",
			files: map[string]string{"test.ts": experimentalE + `declare function make<T>(): { nested: { fn: (x: T) => T } }
export const clean = make<string>()
export const leaky = make<E>()
`},
			want:    []string{"`leaky` exposes `E`"},
			notWant: []string{"`clean` exposes `E`"},
		},
		{
			name: "nested instantiated contexts leaky first",
			files: map[string]string{"test.ts": experimentalE + `declare function make<T>(): { nested: { fn: (x: T) => T } }
export const leaky = make<E>()
export const clean = make<string>()
`},
			want:    []string{"`leaky` exposes `E`"},
			notWant: []string{"`clean` exposes `E`"},
		},
		{
			// P2: an anonymous compound cycle must complete its dependency union
			// for every root rather than caching a cut result as complete.
			name: "mutual cycle A first",
			files: map[string]string{"test.ts": experimentalE + `type B = { c: C; payload: E }
type C = { b: B }
export type A = { b: B }
export type Z = { c: C }
`},
			want: []string{"`A` exposes `E`", "`Z` exposes `E`"},
		},
		{
			name: "mutual cycle Z first",
			files: map[string]string{"test.ts": experimentalE + `type B = { c: C; payload: E }
type C = { b: B }
export type Z = { c: C }
export type A = { b: B }
`},
			want: []string{"`Z` exposes `E`", "`A` exposes `E`"},
		},
		{
			// P2: a nested named interface is a shallow dependency; its arbitrary
			// members are not chased, while its public call signatures are.
			name: "nested named surface stays shallow",
			files: map[string]string{"test.ts": experimentalE + `interface Hidden { value: E }
export declare const exposed: { hidden: Hidden }
`},
			notWant: []string{"exposes"},
		},
		{
			name: "nested named callable signatures are inspected",
			files: map[string]string{"test.ts": experimentalE + `interface Callable { (x: E): void }
export interface Holder { callable: Callable }
`},
			want: []string{"`Holder` exposes `E`"},
		},
		{
			// P2: the computed minimum includes the declared stability of public
			// properties and call signatures, with provenance for naming.
			name: "declared member and signature minimum",
			files: map[string]string{"test.ts": `export interface O {
  /** @stability experimental */
  value: string
}
export interface S {
  /** @stability unstable */
  (x: string): string
}
`},
			want: []string{
				"`O` exposes `value`, an experimental API.",
				"`S` exposes `call signature`, an unstable API.",
			},
		},
		{
			// P2: a default replaced by an explicit argument is not part of the
			// represented instantiated surface.
			name: "replaced generic default stays clean",
			files: map[string]string{"test.ts": experimentalE + `declare function identity<T = E>(x: T): T
export const f = identity<string>
`},
			notWant: []string{"exposes"},
		},
		{
			// Concrete-identity keys: the same raw member type is materialized with
			// different concrete arguments and must not contaminate either way.
			name: "shared raw target clean first",
			files: map[string]string{"test.ts": experimentalE + `type Deep<T> = T extends string ? Deep<T> : T
interface DeepHolder<T> { value: Deep<T> }
export declare const clean: DeepHolder<string>
export declare const leaky: DeepHolder<E>
`},
			want:    []string{"`leaky` exposes `E`"},
			notWant: []string{"`clean` exposes `E`"},
		},
		{
			name: "shared raw target leaky first",
			files: map[string]string{"test.ts": experimentalE + `type Deep<T> = T extends string ? Deep<T> : T
interface DeepHolder<T> { value: Deep<T> }
export declare const leaky: DeepHolder<E>
export declare const clean: DeepHolder<string>
`},
			want:    []string{"`leaky` exposes `E`"},
			notWant: []string{"`clean` exposes `E`"},
		},
		{
			name: "shared heritage materialization clean first",
			files: map[string]string{"test.ts": experimentalE + `interface Base<T> { value: T }
export interface Clean extends Base<string> {}
export interface Leaky extends Base<E> {}
`},
			want:    []string{"`Leaky` exposes `E`"},
			notWant: []string{"`Clean` exposes `E`"},
		},
		{
			name: "shared heritage materialization leaky first",
			files: map[string]string{"test.ts": experimentalE + `interface Base<T> { value: T }
export interface Leaky extends Base<E> {}
export interface Clean extends Base<string> {}
`},
			want:    []string{"`Leaky` exposes `E`"},
			notWant: []string{"`Clean` exposes `E`"},
		},
		{
			// P2: declared generic conditional branches are represented components.
			name: "generic conditional branch is inspected",
			files: map[string]string{"test.ts": experimentalE + `export type Conditional<T> = T extends string ? E : number
`},
			want: []string{"`Conditional` exposes `E`"},
		},
		{
			// P2: an explicit forwarding tag takes precedence over the forwarded
			// symbol's own ceiling, including when it is stricter.
			name: "named forwarding tag tightens ceiling",
			files: map[string]string{
				"test.ts": `/** @stability unstable */
export { Leaky } from "./dep"
`,
				"dep.ts": `/** @stability experimental */
interface E { value: string }
/** @stability experimental */
export interface Leaky { value: E }
`,
			},
			want: []string{"`Leaky` exposes `E`"},
		},
		{
			name: "star forwarding tag tightens ceiling",
			files: map[string]string{
				"test.ts": `/** @stability unstable */
export * from "./dep"
`,
				"dep.ts": `/** @stability experimental */
interface E { value: string }
/** @stability experimental */
export interface Leaky { value: E }
`,
			},
			want: []string{"`Leaky` exposes `E`"},
		},
		{
			name: "forwarding tag below target ceiling stays quiet",
			files: map[string]string{
				"test.ts": `/** @stability experimental */
export { Leaky } from "./dep"
`,
				"dep.ts": `/** @stability experimental */
interface E { value: string }
/** @stability experimental */
export interface Leaky { value: E }
`,
			},
			notWant: []string{"exposes"},
		},
		{
			// P2: erased aliases inside ordinary unions and reference arguments are
			// represented components and keep their provenance.
			name: "erased alias in union and reference argument",
			files: map[string]string{"test.ts": `/** @stability experimental */
type Text = string
interface Box<T> { value: T }
export declare function f(x: Text | undefined): void
export declare const g: Box<Text>
`},
			want: []string{"`f` exposes `Text`", "`g` exposes `Text`"},
		},
		{
			name: "eliminated conditional branch stays clean",
			files: map[string]string{"test.ts": experimentalE + `export type Reduced = string extends string ? number : E
`},
			notWant: []string{"exposes"},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, test.files)
			runApiStabilityRegressionCase(t, test)
		})
	}
}

// TestApiStabilityLeakInstantiationDoesNotScaleWithDepth is the core
// anti-instantiation guard: traversing a deeply nested reference such as
// `Box<Box<...<E>>>` must resolve each represented type argument exactly once,
// so the instantiation delta stays flat as depth grows instead of exploding.
func TestApiStabilityLeakInstantiationDoesNotScaleWithDepth(t *testing.T) {
	t.Parallel()

	deltas := map[int]uint32{}
	for _, depth := range []int{2, 6, 12, 24} {
		source := "/** @stability experimental */\ninterface E { value: string }\ninterface Box<T> { value: T }\n" +
			"export declare const deep: " + strings.Repeat("Box<", depth) + "E" + strings.Repeat(">", depth) + "\n"

		ruleCtx, done := compileApiStabilityFilesPrimed(t, map[string]string{"test.ts": source}, true)
		before := ruleCtx.Checker.TotalInstantiationCount
		messages := runApiStabilityMessages(t, ruleCtx)
		delta := ruleCtx.Checker.TotalInstantiationCount - before
		done()

		if !containsApiStabilityMessage(t, messages, "`deep` exposes `E`") {
			t.Fatalf("depth %d lost its leak diagnostic: %v", depth, messages)
		}
		if delta > apiStabilityMaxInstantiationDelta {
			t.Errorf("depth %d instantiated %d types, want at most %d", depth, delta, apiStabilityMaxInstantiationDelta)
		}
		deltas[depth] = delta
	}
	if deltas[24] > deltas[2]+2 {
		t.Errorf("instantiation delta scaled with depth: depth2=%d depth24=%d", deltas[2], deltas[24])
	}
}

// TestApiStabilityLeakDeferredBranchRegressions pins non-instantiating
// declaration access for conditional branches and value
// annotations, substitution-aware branch selection, deferred-branch erased
// aliases and composite deferred checks. Every case runs on warm and genuinely
// cold checkers with the instantiation and diagnostic guards of
// runApiStabilityRegressionCase, so a recursive alias argument can neither be
// evaluated nor change the compiler's diagnostics.
func TestApiStabilityLeakDeferredBranchRegressions(t *testing.T) {
	t.Parallel()

	deepAlias := "type Deep<T> = T extends string ? Deep<T> : T\n"

	tests := []apiStabilityRegressionCase{
		{
			// A composite check that contains a type parameter keeps its
			// declared branches instead of being published blocked.
			name:  "generic conditional array check",
			files: singleApiStabilityFile(`export type C<T> = T[] extends string[] ? E : number`),
			want:  []string{"`C` exposes `E`"},
		},
		{
			// The represented substitution decides which branch survives; an
			// eliminated experimental branch is not reported.
			name: "interface instantiated conditional eliminates branch",
			files: singleApiStabilityFile(`interface Holder<T> { value: T extends string ? number : E }
export declare const clean: Holder<string>
export declare const leaky: Holder<E>`),
			want:    []string{"`leaky` exposes `E`"},
			notWant: []string{"`clean` exposes `E`"},
		},
		{
			// The same selection holds for a named alias whose declared surface
			// is walked from its annotation on a cold checker.
			name: "alias instantiated conditional eliminates branch",
			files: singleApiStabilityFile(`type Holder<T> = { value: T extends string ? number : E }
type Direct<T> = T extends string ? number : E
export declare const clean: Holder<string>
export declare const leaky: Holder<E>
export declare const directClean: Direct<string>
export declare const directLeaky: Direct<E>`),
			want:    []string{"`leaky` exposes `E`", "`directLeaky` exposes `E`"},
			notWant: []string{"`clean` exposes `E`", "`directClean` exposes `E`"},
		},
		{
			// A deferred branch that survives keeps the erased alias written in
			// its annotation.
			name: "deferred conditional erased branch alias",
			files: map[string]string{"test.ts": `/** @stability experimental */
type Text = string
export type C<T> = T extends number ? Text : boolean`},
			want: []string{"`C` exposes `Text`"},
		},
		{
			// A declared reference whose argument is a recursive alias is never
			// resolved by the rule.
			name: "declared reference recursive argument budget",
			files: map[string]string{"test.ts": deepAlias + `interface Callable<T> { (x: T): void }
export declare const f: Callable<Deep<string>>`},
			notWant: []string{"exposes"},
		},
		{
			// A deferred branch that instantiates a recursive alias is read
			// from its annotation, never evaluated.
			name:    "branch recursive concrete alias budget",
			files:   map[string]string{"test.ts": deepAlias + `export type C<T> = T extends number ? Deep<string> : boolean`},
			notWant: []string{"exposes"},
		},
		{
			// The cold callable probe: an initializer call whose return type
			// carries a recursive alias argument is read through the resolved
			// signature's uninstantiated return type and concrete substitution.
			name: "recursive named callable argument budget",
			files: map[string]string{"test.ts": deepAlias + `interface Callable<T> { (x: T): void }
declare function make<T>(): Callable<Deep<T>>
export const f = make<string>()`},
			notWant: []string{"exposes"},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, test.files)
			runApiStabilityRegressionCase(t, test)
		})
	}
}

// TestApiStabilityLeakCyclicCompletionIsReusable compiles the mutual anonymous
// cycle in both export orders, repeatedly and on warm checkers, and requires
// every root to report the experimental dependency. A cut result must never be
// reported as stable, and a settled result must be reusable within its session.
func TestApiStabilityLeakCyclicCompletionIsReusable(t *testing.T) {
	t.Parallel()

	orders := []string{
		"export type A={b:B;payload:E}\nexport type B={a:A}\n",
		"export type B={a:A}\nexport type A={b:B;payload:E}\n",
	}
	for index, body := range orders {
		t.Run(fmt.Sprintf("order=%d", index), func(t *testing.T) {
			t.Parallel()
			files := map[string]string{"test.ts": `/** @stability experimental */
interface E {value:string}
` + body}
			for iteration := range 10 {
				ruleCtx, done := compileApiStabilityFilesPrimed(t, files, true)
				messages := runApiStabilityMessages(t, ruleCtx)
				done()
				if !containsApiStabilityMessage(t, messages, "`A` exposes `E`") ||
					!containsApiStabilityMessage(t, messages, "`B` exposes `E`") {
					t.Fatalf("iteration=%d lost a cyclic diagnostic: %v", iteration, messages)
				}
			}
		})
	}
}

// TestApiStabilityNonInstantiatingAccessors pins that the checker accessors the
// analysis hides behind only peek cached fields: a conditional branch, an index
// signature value and a symbol or alias type are never resolved, so a recursive
// alias argument such as `Deep<string>` cannot be instantiated by them.
func TestApiStabilityNonInstantiatingAccessors(t *testing.T) {
	t.Parallel()

	for _, branch := range []string{"Box<string>", "Select<string>", "Deep<string>"} {
		t.Run("branch/"+branch, func(t *testing.T) {
			t.Parallel()
			source := `interface Box<T>{value:T}
type Select<T> = T extends string ? {value:T} : T
type Deep<T> = T extends string ? Deep<T> : T
export type C<T> = T extends number ? ` + branch + ` : boolean`
			ruleCtx, done := compileApiStabilityFilesPrimed(t, map[string]string{"test.ts": source}, false)
			defer done()

			exports := ruleCtx.Checker.GetExportsOfModule(ruleCtx.Checker.GetSymbolAtLocation(ruleCtx.SourceFile.AsNode()))
			if len(exports) != 1 {
				t.Fatalf("got %d exports, want 1", len(exports))
			}
			conditional := ruleCtx.Checker.GetDeclaredTypeOfSymbol(exports[0])
			if conditional.Flags()&checker.TypeFlagsConditional == 0 {
				t.Fatal("declared type is not a conditional")
			}
			before := ruleCtx.Checker.TotalInstantiationCount
			if resolved := checker.GetResolvedConditionalTypeBranch(ruleCtx.Checker, conditional, true); resolved != nil {
				t.Fatal("cold conditional branch should not be resolved")
			}
			_ = checker.GetConditionalTypeBranchType(ruleCtx.Checker, conditional, true)
			if delta := ruleCtx.Checker.TotalInstantiationCount - before; delta != 0 {
				t.Errorf("branch accessor instantiated %d types, want 0", delta)
			}
			if globals := ruleCtx.Checker.GetGlobalDiagnostics(); len(globals) != 0 {
				t.Errorf("branch accessor added global diagnostics: %v", globals)
			}
		})
	}

	t.Run("index infos", func(t *testing.T) {
		t.Parallel()
		source := `type Deep<T> = T extends string ? Deep<T> : T
export interface I<T> {[key:string]:Deep<string>}`
		ruleCtx, done := compileApiStabilityFilesPrimed(t, map[string]string{"test.ts": source}, false)
		defer done()

		exports := ruleCtx.Checker.GetExportsOfModule(ruleCtx.Checker.GetSymbolAtLocation(ruleCtx.SourceFile.AsNode()))
		if len(exports) != 1 {
			t.Fatalf("got %d exports, want 1", len(exports))
		}
		before := ruleCtx.Checker.TotalInstantiationCount
		_ = checker.Checker_getMembersOfSymbol(ruleCtx.Checker, exports[0])
		infos := checker.GetDeclaredIndexInfosOfSymbol(ruleCtx.Checker, exports[0])
		if delta := ruleCtx.Checker.TotalInstantiationCount - before; delta != 0 {
			t.Errorf("index accessor instantiated %d types, want 0", delta)
		}
		for _, info := range infos {
			if info != nil && info.ValueType() != nil {
				t.Errorf("unresolved index value type should stay nil")
			}
		}
		if globals := ruleCtx.Checker.GetGlobalDiagnostics(); len(globals) != 0 {
			t.Errorf("index accessor added global diagnostics: %v", globals)
		}
	})

	t.Run("symbol and alias types", func(t *testing.T) {
		t.Parallel()
		source := `type Deep<T> = T extends string ? Deep<T> : T
type S = Deep<string>
export declare const x: S`
		ruleCtx, done := compileApiStabilityFilesPrimed(t, map[string]string{"test.ts": source}, false)
		defer done()

		exports := ruleCtx.Checker.GetExportsOfModule(ruleCtx.Checker.GetSymbolAtLocation(ruleCtx.SourceFile.AsNode()))
		if len(exports) != 1 {
			t.Fatalf("got %d exports, want 1", len(exports))
		}
		before := ruleCtx.Checker.TotalInstantiationCount
		if materialized := checker.GetResolvedTypeOfSymbolIfMaterialized(ruleCtx.Checker, exports[0]); materialized != nil {
			t.Fatalf("cold export value type should not be materialized")
		}
		_ = checker.GetResolvedDeclaredTypeOfSymbolIfMaterialized(ruleCtx.Checker, exports[0])
		if delta := ruleCtx.Checker.TotalInstantiationCount - before; delta != 0 {
			t.Errorf("symbol accessor instantiated %d types, want 0", delta)
		}
		if globals := ruleCtx.Checker.GetGlobalDiagnostics(); len(globals) != 0 {
			t.Errorf("symbol accessor added global diagnostics: %v", globals)
		}
	})
}

// TestApiStabilityLeakConditionalApplicationRegressions pins annotated
// recursive declarations are read without evaluating a generic alias, deferred
// conditional applications distinguish concrete substitutions, declared member
// and signature tags contribute on cold checkers, `typeof` callable surfaces
// keep their signature dependencies, and eliminated `string[]`/`never`
// conditional branches stay clean. Every case runs on warm and genuinely cold
// checkers with the instantiation and compiler-diagnostic guards of
// runApiStabilityRegressionCase.
func TestApiStabilityLeakConditionalApplicationRegressions(t *testing.T) {
	t.Parallel()

	deepAlias := "type Deep<T> = T extends string ? Deep<T> : T\n"
	stepAlias := "type Step<T extends unknown[]> = T extends [unknown, ...infer Tail] ? Step<Tail> : number\n"

	tests := []apiStabilityRegressionCase{
		// P1: an annotated recursive declaration is never resolved, on any
		// declaration surface (parameter, return, member, inherited member).
		{
			name:    "annotated recursive parameter",
			files:   map[string]string{"test.ts": deepAlias + `export declare function f(x: Deep<string>): void`},
			notWant: []string{"exposes"},
		},
		{
			name:    "annotated recursive return",
			files:   map[string]string{"test.ts": deepAlias + `export declare function f(): Deep<string>`},
			notWant: []string{"exposes"},
		},
		{
			name:    "annotated recursive declared property",
			files:   map[string]string{"test.ts": deepAlias + `export interface I { value: Deep<string> }`},
			notWant: []string{"exposes"},
		},
		{
			name:    "annotated recursive inherited method",
			files:   map[string]string{"test.ts": deepAlias + `interface Base { f(x: Deep<string>): void }` + "\n" + `export interface Child extends Base {}`},
			notWant: []string{"exposes"},
		},
		{
			// A finite recursive tuple application stays deferred as well; it
			// must not be instantiated just because it terminates.
			name: "finite recursive tuple application",
			files: map[string]string{"test.ts": stepAlias +
				`export declare function f(x: Step<[` + strings.Repeat("0,", 60) + `]>): void`},
			notWant: []string{"exposes"},
		},

		// P2: distinct concrete applications of a recursive alias are
		// distinguished, so the eliminated branch of the finite alias is still
		// inspected on a cold checker.
		{
			name: "finite recursive alias application",
			files: singleApiStabilityFile(`type A<T> = T extends string ? A<number> : E
export declare const x: A<string>`),
			want: []string{"`x` exposes `E`"},
		},
		{
			// A generic alias whose right-hand side is a named interface keeps
			// the concrete argument when the referenced declaration is read.
			name: "alias of interface preserves argument",
			files: singleApiStabilityFile(`interface Box<T> { value: T }
type Alias<T> = Box<T>
export declare const x: Alias<E>`),
			want: []string{"`x` exposes `E`"},
		},
		{
			name: "alias of type literal preserves argument",
			files: singleApiStabilityFile(`type Wrapper<T> = { value: T }
export declare const y: Wrapper<E>`),
			want: []string{"`y` exposes `E`"},
		},
		{
			// An application the checker evaluates to `never` exposes neither
			// its operands nor its branches.
			name: "eliminated conditional alias application",
			files: singleApiStabilityFile(`type Hidden<T> = T extends string ? E : never
export declare const clean: Hidden<E>`),
			notWant: []string{"exposes"},
		},

		// P2: declared member and signature tags contribute even when the
		// checker has not materialized the annotation they belong to.
		{
			name: "cold tagged anonymous property",
			files: map[string]string{"test.ts": `export declare const o: {
/** @stability experimental */
value: string
}`},
			want: []string{"exposes"},
		},
		{
			name: "cold tagged anonymous method",
			files: map[string]string{"test.ts": `export declare const o: {
/** @stability experimental */
f(x: string): void
}`},
			want: []string{"exposes"},
		},
		{
			name: "cold tagged anonymous call signature",
			files: map[string]string{"test.ts": `export declare const o: {
/** @stability experimental */
(x: string): void
}`},
			want: []string{"exposes"},
		},
		{
			name: "cold tagged conditional branch member",
			files: map[string]string{"test.ts": `export type C<T> = T extends number ? {
/** @stability experimental */
value: string
} : boolean`},
			want: []string{"exposes"},
		},

		// P2: a `typeof` callable keeps its public signature dependencies
		// without resolving the referenced function's parameter recursively.
		{
			name: "cold typeof callable",
			files: singleApiStabilityFile(`declare function fn(x: E): void` + "\n" +
				`export declare const o: typeof fn`),
			want: []string{"`o` exposes `E`"},
		},
		{
			// The same callable surface keeps its budget when the referenced
			// signature parameter is a recursive alias application.
			name: "cold typeof recursive callable",
			files: map[string]string{"test.ts": deepAlias +
				`declare function fn(x: Deep<string>): void` + "\n" +
				`export declare const o: typeof fn`},
			notWant: []string{"exposes"},
		},
		{
			// A tagged member of an anonymous surface is reported even when its
			// own type is a recursive alias application that must stay deferred.
			name: "cold tagged anonymous recursive member",
			files: map[string]string{"test.ts": deepAlias + `export declare const o: {
/** @stability experimental */
value: Deep<string>
}`},
			want: []string{"exposes"},
		},

		// P2: a concrete compound substitution is preserved so an eliminated
		// conditional branch is not reported; `never` distributes to no branch.
		{
			name: "compound argument conditional eliminated",
			files: singleApiStabilityFile(`interface Holder<T> { value: T extends string[] ? number : E }` + "\n" +
				`export declare const clean: Holder<string[]>`),
			notWant: []string{"exposes"},
		},
		{
			name: "never distributive conditional eliminated",
			files: singleApiStabilityFile(`interface Holder<T> { value: T extends string ? E : number }` + "\n" +
				`export declare const clean: Holder<never>`),
			notWant: []string{"exposes"},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, test.files)
			runApiStabilityRegressionCase(t, test)
		})
	}
}

// TestApiStabilityLeakColdLazyReadMatchesWarm proves a cold public surface is
// materialized through the checker's ordinary lazy reads and agrees with the
// materialized recomputation: `Callable<Deep<E>>` resolves E on the cold
// checker (the application terminates because E is not a string), and the
// warmed recomputation reports the same dependency from a fresh analysis.
func TestApiStabilityLeakColdLazyReadMatchesWarm(t *testing.T) {
	t.Parallel()

	files := map[string]string{"test.ts": `/** @stability experimental */
interface E { value: string }
type Deep<T> = T extends string ? Deep<T> : T
interface Callable<T> { (x: T): void }
export declare const f: Callable<Deep<E>>
`}
	ruleCtx, done := compileApiStabilityFilesPrimed(t, files, false)
	defer done()

	exports := ruleCtx.Checker.GetExportsOfModule(ruleCtx.Checker.GetSymbolAtLocation(ruleCtx.SourceFile.AsNode()))
	if len(exports) != 1 {
		t.Fatalf("got %d exports, want 1", len(exports))
	}
	symbol := exports[0]

	cold := ruleCtx.TypeParser.ApiStabilityUsedBySymbol(symbol)
	if !containsDependencyName(cold, "E") {
		t.Fatalf("the cold surface should lazily resolve E, got %v", cold.Dependencies)
	}

	_ = apiStabilityCompilerDiagnostics(ruleCtx)
	warm := ruleCtx.TypeParser.ApiStabilityUsedBySymbol(symbol)
	if !containsDependencyName(warm, "E") {
		t.Fatalf("warm surface should expose E, got %v", warm.Dependencies)
	}
	if cold.Minimum != warm.Minimum || len(cold.Dependencies) != len(warm.Dependencies) {
		t.Fatalf("cold and warm surfaces disagree: cold=%v warm=%v", cold.Dependencies, warm.Dependencies)
	}
}

// containsDependencyName reports whether a computed surface lists the named
// dependency, used by the cold/warm recompute regression.
func containsDependencyName(used typeparser.ApiStabilityUsed, name string) bool {
	for _, dependency := range used.Dependencies {
		if typeparser.ApiStabilityDependencyName(dependency) == name {
			return true
		}
	}
	return false
}

// apiStabilityColdUnavailableProbes groups the cold-unavailable probes whose
// public types are unavailable on a genuinely unprimed checker. None of them
// may trigger checker instantiation, produce a compiler diagnostic, report a
// fabricated dependency, or publish a complete cache entry.
func apiStabilityColdUnavailableProbes() []struct {
	name   string
	source string
} {
	deepAlias := "type Deep<T> = T extends string ? Deep<T> : T\n"
	return []struct {
		name   string
		source string
	}{
		{"indexed recursive parameter", deepAlias + `interface I { x: Deep<string> }
export declare function f(x: I["x"]): void`},
		{"keyof recursive mapped", deepAlias + `interface I extends Deep<string> {}
export declare function f(x: keyof I): void`},
		{"anonymous recursive index", deepAlias + `export declare function f(): { [key: string]: Deep<string> }`},
		{"recursive generic default parameter", deepAlias + `interface I<T = Deep<string>> { x: T }
export declare function f(x: I): void`},
		{"recursive generic default return", deepAlias + `interface I<T = Deep<string>> { x: T }
export declare function f(): I`},
		{"inferred getter", deepAlias + `declare const deep: Deep<string>
export class C { get x() { return deep } }`},
		{"inferred parameter initializer", deepAlias + `declare const deep: Deep<string>
export function f(x = deep): void {}`},
		{"inferred return", deepAlias + `declare const deep: Deep<string>
export function f() { return deep }`},
		{"anonymous nested inferred", deepAlias + `declare const deep: Deep<string>
export const x = { nested: { deep } }`},
		{"mapped template", deepAlias + `export type M = { [K in "x"]: Deep<string> }`},
		{"mapped name", deepAlias + `export type M = { [K in "x" as Deep<string>]: number }`},
		{"type literal alias index", deepAlias + `export type M = { [x: string]: Deep<string> }`},
	}
}

// TestApiStabilityLeakColdUnavailableIsSafe runs every recursive probe whose
// public type is not materialized on an unprimed checker. The analysis must do
// zero recursive work: no checker instantiation, no global diagnostic, no
// fabricated dependency, and no analysis state that survives the run.
func TestApiStabilityLeakColdUnavailableIsSafe(t *testing.T) {
	t.Parallel()

	for _, test := range apiStabilityColdUnavailableProbes() {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			files := map[string]string{"test.ts": test.source}
			ruleCtx, done := compileApiStabilityFilesPrimed(t, files, false)
			before := ruleCtx.Checker.TotalInstantiationCount
			messages := runApiStabilityMessages(t, ruleCtx)
			delta := ruleCtx.Checker.TotalInstantiationCount - before
			compiler := apiStabilityCompilerDiagnostics(ruleCtx)
			globals := len(ruleCtx.Checker.GetGlobalDiagnostics())
			done()

			if delta > apiStabilityMaxInstantiationDelta {
				t.Errorf("cold rule instantiated %d types, want at most %d", delta, apiStabilityMaxInstantiationDelta)
			}
			if globals != 0 {
				t.Errorf("cold rule produced %d global diagnostics", globals)
			}
			if len(messages) != 0 {
				t.Errorf("cold rule fabricated diagnostics: %v", messages)
			}
			if control := apiStabilityControlCompilerDiagnostics(t, files); !slices.Equal(control, compiler) {
				t.Errorf("cold rule changed compiler diagnostics against control:\ncontrol=%v\nafter=%v", control, compiler)
			}
		})
	}
}

// TestApiStabilityLeakWarmReanalysisIsExact re-runs the same probes on a
// checked checker, where the normal rule lifecycle makes the public types
// available. Each repro must then report exactly its expected result.
func TestApiStabilityLeakWarmReanalysisIsExact(t *testing.T) {
	t.Parallel()

	prefix := "/** @stability experimental */\ninterface E { value: string }\n"
	cases := []struct {
		name    string
		source  string
		want    []string
		notWant []string
	}{
		{
			name: "typeof inferred arrow",
			source: `declare const e: E
const fn = () => e
export declare const x: typeof fn`,
			want: []string{"`x` exposes `E`"},
		},
		{
			name: "typeof inferred nested closure",
			source: `declare const e: E
const fn = () => () => e
export declare const x: typeof fn`,
			want: []string{"`x` exposes `E`"},
		},
		{
			name: "indexed erased dependency",
			source: `interface I { value: E }
type Read<T> = T["value" & keyof T]
export declare const x: Read<I>`,
			want: []string{"`x` exposes `E`"},
		},
		{
			name: "indexed recursive parameter stays clean",
			source: `type Deep<T> = T extends string ? Deep<T> : T
interface I { x: Deep<string> }
export declare function f(x: I["x"]): void`,
			notWant: []string{"exposes"},
		},
		{
			name: "anonymous recursive index stays clean",
			source: `type Deep<T> = T extends string ? Deep<T> : T
export declare function f(): { [key: string]: Deep<string> }`,
			notWant: []string{"exposes"},
		},
		{
			name: "recursive generic default parameter stays clean",
			source: `type Deep<T> = T extends string ? Deep<T> : T
interface I<T = Deep<string>> { x: T }
export declare function f(x: I): void`,
			notWant: []string{"exposes"},
		},
		{
			name: "recursive generic default return stays clean",
			source: `type Deep<T> = T extends string ? Deep<T> : T
interface I<T = Deep<string>> { x: T }
export declare function f(): I`,
			notWant: []string{"exposes"},
		},
		{
			name: "default conditional interface stays clean",
			source: `interface Holder<T = string> { value: T extends string ? number : E }
export declare const x: Holder`,
			notWant: []string{"`x` exposes `E`"},
		},
		{
			name: "default conditional alias stays clean",
			source: `type Holder<T = string> = T extends string ? number : E
export declare const x: Holder`,
			notWant: []string{"`x` exposes `E`"},
		},
		{
			name: "eliminated conditional alias application stays clean",
			source: `type Hidden<T> = T extends string ? E : never
export declare const clean: Hidden<E>`,
			notWant: []string{"exposes"},
		},
		{
			name: "finite recursive alias application reports",
			source: `type A<T> = T extends string ? A<number> : E
export declare const x: A<string>`,
			want: []string{"`x` exposes `E`"},
		},
	}

	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			ruleCtx, done := compileApiStabilityFilesPrimed(t, map[string]string{"test.ts": prefix + test.source}, true)
			defer done()

			before := apiStabilityCompilerDiagnostics(ruleCtx)
			messages := runApiStabilityMessages(t, ruleCtx)
			after := apiStabilityCompilerDiagnostics(ruleCtx)
			if !slices.Equal(before, after) {
				t.Fatalf("warm rule changed compiler diagnostics:\nbefore=%v\nafter=%v", before, after)
			}
			for _, want := range test.want {
				if !containsApiStabilityMessage(t, messages, want) {
					t.Errorf("missing diagnostic %q in %v", want, messages)
				}
			}
			for _, notWant := range test.notWant {
				if containsApiStabilityMessage(t, messages, notWant) {
					t.Errorf("unexpected diagnostic %q in %v", notWant, messages)
				}
			}
		})
	}
}

// TestApiStabilityLeakRepresentedConditionalOutcomes proves the analysis uses a
// conditional outcome the checker has materialized instead of guessing a
// branch. The instantiated member type is requested first, the way a consumer
// of the exported value would, and the rule must then reproduce the compiler's
// selection for contravariant, invariant, array-wrapped and structural checks.
func TestApiStabilityLeakRepresentedConditionalOutcomes(t *testing.T) {
	t.Parallel()

	prefix := "/** @stability experimental */\ninterface E { value: string }\n"
	cases := []struct {
		name   string
		source string
		leak   bool
	}{
		{
			name: "contravariant conditional",
			source: `interface C<in T> { f: (x: T) => void }
interface Holder<T> { value: T extends C<string> ? number : E }
export declare const x: Holder<C<"x">>`,
			leak: true,
		},
		{
			name: "invariant conditional",
			source: `interface C<in out T> { f: (x: T) => T }
interface Holder<T> { value: T extends C<string> ? number : E }
export declare const x: Holder<C<"x">>`,
			leak: true,
		},
		{
			name: "array wrapped conditional",
			source: `interface C<in T> { f: (x: T) => void }
interface Holder<T> { value: T extends C<string>[] ? number : E }
export declare const x: Holder<C<"x">[]>`,
			leak: true,
		},
		{
			name: "structural conditional",
			source: `interface Holder<T> { value: T extends { value: string } ? number : E }
export declare const x: Holder<{ value: string }>`,
			leak: false,
		},
	}

	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			ruleCtx, done := compileApiStabilityFilesPrimed(t, map[string]string{"test.ts": prefix + test.source}, true)
			defer done()

			// Fully represent the conditional outcome: ask the checker for the
			// instantiated member type, which materializes the selected branch.
			for _, symbol := range ruleCtx.Checker.GetExportsOfModule(ruleCtx.Checker.GetSymbolAtLocation(ruleCtx.SourceFile.AsNode())) {
				if symbol.Name != "x" {
					continue
				}
				value := ruleCtx.Checker.GetTypeOfSymbol(symbol)
				for _, property := range ruleCtx.Checker.GetPropertiesOfType(value) {
					if property.Name == "value" {
						_ = ruleCtx.Checker.GetTypeOfSymbol(property)
					}
				}
			}

			messages := runApiStabilityMessages(t, ruleCtx)
			if test.leak {
				if !containsApiStabilityMessage(t, messages, "`x` exposes `E`") {
					t.Errorf("missing diagnostic in %v", messages)
				}
			} else if containsApiStabilityMessage(t, messages, "`x` exposes `E`") {
				t.Errorf("unexpected diagnostic in %v", messages)
			}
		})
	}
}

// TestApiStabilityLeakLazyConditionalOutcomes proves the corrected lazy-read
// boundary for instantiated conditional members: the concrete member is
// materialized through the checker's ordinary accessor, so the selected branch
// is the compiler's outcome rather than a guess. A contravariant check selects
// the experimental branch and reports; a structural check selects the clean
// branch and stays quiet. Both outcomes are complete surfaces, on warm and
// genuinely cold checkers, and a cold run never changes the compiler's own
// diagnostics.
func TestApiStabilityLeakLazyConditionalOutcomes(t *testing.T) {
	t.Parallel()

	prefix := "/** @stability experimental */\ninterface E { value: string }\n"
	cases := []struct {
		name   string
		source string
		leak   bool
	}{
		{
			name: "contravariant conditional selected branch",
			source: `interface C<in T> { f: (x: T) => void }
interface Holder<T> { value: T extends C<string> ? number : E }
export declare const x: Holder<C<"x">>`,
			leak: true,
		},
		{
			name: "structural conditional selected branch",
			source: `interface Holder<T> { value: T extends { value: string } ? number : E }
export declare const x: Holder<{ value: string }>`,
			leak: false,
		},
	}

	for _, test := range cases {
		for _, cold := range []bool{false, true} {
			name := test.name
			if cold {
				name += "/cold"
			} else {
				name += "/warm"
			}
			t.Run(name, func(t *testing.T) {
				t.Parallel()
				files := map[string]string{"test.ts": prefix + test.source}
				ruleCtx, done := compileApiStabilityFilesPrimed(t, files, !cold)
				messages := runApiStabilityMessages(t, ruleCtx)
				compiler := apiStabilityCompilerDiagnostics(ruleCtx)
				session := ruleCtx.TypeParser.NewApiStabilitySession()
				incomplete := false
				for _, symbol := range ruleCtx.Checker.GetExportsOfModule(ruleCtx.Checker.GetSymbolAtLocation(ruleCtx.SourceFile.AsNode())) {
					if used := session.UsedBySymbol(symbol); used.Incomplete {
						incomplete = true
					}
				}
				done()
				if got := containsApiStabilityMessage(t, messages, "`x` exposes `E`"); got != test.leak {
					t.Errorf("leak=%v want=%v: %v", got, test.leak, messages)
				}
				if incomplete {
					t.Error("a resolved conditional outcome should be a complete surface")
				}
				if cold {
					if control := apiStabilityControlCompilerDiagnostics(t, files); !slices.Equal(control, compiler) {
						t.Errorf("cold rule changed compiler diagnostics against control:\ncontrol=%v\nafter=%v", control, compiler)
					}
				}
			})
		}
	}
}

// TestApiStabilityLeakDeclaredIndexSignatureTags pins the index signature tag
// fix: the signature's own `@stability` tag contributes on both the warm
// represented path and the cold metadata path, for a declared interface and an
// inline type literal.
func TestApiStabilityLeakDeclaredIndexSignatureTags(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name   string
		source string
	}{
		{
			name: "declared interface index",
			source: `export interface I {
  /** @stability experimental */
  [key: string]: number
}`,
		},
		{
			name: "inline type literal index",
			source: `export declare const x: {
  /** @stability experimental */
  [key: string]: number
}`,
		},
	}

	for _, test := range cases {
		for _, cold := range []bool{false, true} {
			name := test.name
			if cold {
				name += "/cold"
			} else {
				name += "/warm"
			}
			t.Run(name, func(t *testing.T) {
				t.Parallel()
				ruleCtx, done := compileApiStabilityFilesPrimed(t, map[string]string{"test.ts": test.source}, !cold)
				defer done()
				messages := runApiStabilityMessages(t, ruleCtx)
				if !containsApiStabilityMessage(t, messages, "exposes") {
					t.Fatalf("missing index signature tag diagnostic in %v", messages)
				}
			})
		}
	}
}

// TestApiStabilityLeakFiniteRecursionDoesNotInstantiate is the
// finite-recursion anti-instantiation guard for valid finite recursion: a 60-element `Step`
// application reached through an indexed parameter or through inferred getter,
// parameter, return and nested object surfaces must not make the rule
// instantiate anything on an unprimed checker, and must not produce a compiler
// diagnostic.
func TestApiStabilityLeakFiniteRecursionDoesNotInstantiate(t *testing.T) {
	t.Parallel()

	step := "type Step<T extends unknown[]> = T extends [unknown, ...infer Tail] ? Step<Tail> : number\n"
	application := "declare const deep: Step<[" + strings.Repeat("0,", 60) + "]>\n"
	cases := []struct {
		name   string
		source string
	}{
		{"indexed parameter", step + `interface I { value: Step<[` + strings.Repeat("0,", 60) + `]> }
export declare function f(x: I["value"]): void`},
		{"inferred getter", step + application + `export class C { get x() { return deep } }`},
		{"inferred parameter initializer", step + application + `export function f(x = deep): void {}`},
		{"inferred return", step + application + `export function f() { return deep }`},
		{"anonymous nested inferred", step + application + `export const x = { nested: { deep } }`},
	}

	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			files := map[string]string{"test.ts": test.source}
			ruleCtx, done := compileApiStabilityFilesPrimed(t, files, false)
			before := ruleCtx.Checker.TotalInstantiationCount
			messages := runApiStabilityMessages(t, ruleCtx)
			delta := ruleCtx.Checker.TotalInstantiationCount - before
			compiler := apiStabilityCompilerDiagnostics(ruleCtx)
			globals := len(ruleCtx.Checker.GetGlobalDiagnostics())
			done()

			if delta > apiStabilityMaxInstantiationDelta {
				t.Errorf("rule instantiated %d types, want at most %d", delta, apiStabilityMaxInstantiationDelta)
			}
			if globals != 0 {
				t.Errorf("rule produced %d global diagnostics", globals)
			}
			if len(messages) != 0 {
				t.Errorf("rule produced diagnostics: %v", messages)
			}
			if control := apiStabilityControlCompilerDiagnostics(t, files); !slices.Equal(control, compiler) {
				t.Errorf("cold rule changed compiler diagnostics against control:\ncontrol=%v\nafter=%v", control, compiler)
			}
		})
	}
}
