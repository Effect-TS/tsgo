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
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/bundled"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

func compileApiStabilityFiles(t *testing.T, files map[string]string) (*rule.Context, func()) {
	t.Helper()
	return compileApiStabilityFilesPrimed(t, files, true)
}

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

const apiStabilityMaxInstantiationDelta = 100_000

func apiStabilityControlCompilerDiagnostics(t *testing.T, files map[string]string) []string {
	t.Helper()
	ruleCtx, done := compileApiStabilityFilesPrimed(t, files, false)
	defer done()
	return apiStabilityCompilerDiagnostics(ruleCtx)
}

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

func apiStabilityBothOrders(name, prefix, clean, leaky string, notWant string) []apiStabilityRegressionCase {
	return []apiStabilityRegressionCase{
		{
			name:    name + " clean first",
			files:   map[string]string{"test.ts": prefix + clean + leaky},
			want:    []string{"`leaky` exposes `E`"},
			notWant: []string{notWant},
		},
		{
			name:    name + " leaky first",
			files:   map[string]string{"test.ts": prefix + leaky + clean},
			want:    []string{"`leaky` exposes `E`"},
			notWant: []string{notWant},
		},
	}
}

func TestApiStabilityLeakColdSafety(t *testing.T) {
	t.Parallel()

	experimentalE := `/** @stability experimental */
interface E { value: string }
`
	deepAlias := "type Deep<T> = T extends string ? Deep<T> : T\n"
	stepAlias := "type Step<T extends unknown[]> = T extends [unknown, ...infer Tail] ? Step<Tail> : number\n"

	tests := []apiStabilityRegressionCase{
		{
			name: "recursive member budget",
			files: map[string]string{"test.ts": deepAlias + `interface Wrapper<T> { value: Deep<T> }
export declare const exposed: Wrapper<string>
`},
			notWant: []string{"exposes"},
		},
		{
			name: "inferred getter budget",
			files: map[string]string{"test.ts": deepAlias + `interface Callable<T> { (x: Deep<T>): void }
declare const budgetValue: Callable<string>
export class Owner { get method() { return budgetValue } }
`},
			notWant: []string{"exposes"},
		},
		{
			name: "inferred getter return budget",
			files: map[string]string{"test.ts": deepAlias + `interface Callable<T> { (): Deep<T> }
declare const budgetValue: Callable<string>
export class Owner { get method() { return budgetValue } }
`},
			notWant: []string{"exposes"},
		},
		{
			name: "referenced non-generic getter budget",
			files: map[string]string{"test.ts": deepAlias + `interface Callable<T> { (x: Deep<T>): void }
declare const budgetValue: Callable<string>
class Hidden { get method() { return budgetValue } }
export declare const owner: Hidden
`},
			notWant: []string{"exposes"},
		},
		{
			name: "instantiated anonymous object budget",
			files: map[string]string{"test.ts": deepAlias + `declare function make<T>(): { value: Deep<T> }
export const object = make<string>()
`},
			notWant: []string{"exposes"},
		},
		{
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
			name:    "declared reference recursive argument budget",
			files:   map[string]string{"test.ts": deepAlias + "interface Callable<T> { (x: T): void }\nexport declare const f: Callable<Deep<string>>"},
			notWant: []string{"exposes"},
		},
		{
			name:    "branch recursive concrete alias budget",
			files:   map[string]string{"test.ts": deepAlias + `export type C<T> = T extends number ? Deep<string> : boolean`},
			notWant: []string{"exposes"},
		},
		{
			name:    "recursive named callable argument budget",
			files:   map[string]string{"test.ts": deepAlias + "interface Callable<T> { (x: T): void }\ndeclare function make<T>(): Callable<Deep<T>>\nexport const f = make<string>()"},
			notWant: []string{"exposes"},
		},
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
			files:   map[string]string{"test.ts": deepAlias + "interface Base { f(x: Deep<string>): void }\nexport interface Child extends Base {}"},
			notWant: []string{"exposes"},
		},
		{
			name:    "indexed recursive parameter",
			files:   map[string]string{"test.ts": deepAlias + "interface I { x: Deep<string> }\nexport declare function f(x: I[\"x\"]): void"},
			notWant: []string{"exposes"},
		},
		{
			name:    "anonymous recursive index",
			files:   map[string]string{"test.ts": deepAlias + `export declare function f(): { [key: string]: Deep<string> }`},
			notWant: []string{"exposes"},
		},
		{
			name:    "recursive generic default parameter",
			files:   map[string]string{"test.ts": deepAlias + "interface I<T = Deep<string>> { x: T }\nexport declare function f(x: I): void"},
			notWant: []string{"exposes"},
		},
		{
			name:    "recursive generic default return",
			files:   map[string]string{"test.ts": deepAlias + "interface I<T = Deep<string>> { x: T }\nexport declare function f(): I"},
			notWant: []string{"exposes"},
		},
		{
			name:    "cold typeof recursive callable",
			files:   map[string]string{"test.ts": deepAlias + "declare function fn(x: Deep<string>): void\nexport declare const o: typeof fn"},
			notWant: []string{"exposes"},
		},
		{
			name: "cold tagged anonymous recursive member",
			files: map[string]string{"test.ts": deepAlias + `export declare const o: {
/** @stability experimental */
value: Deep<string>
}`},
			want: []string{"exposes"},
		},
		{
			name:    "finite recursive tuple application",
			files:   map[string]string{"test.ts": stepAlias + `export declare function f(x: Step<[` + strings.Repeat("0,", 60) + `]>): void`},
			notWant: []string{"exposes"},
		},
		{
			name:    "replaced generic default is not reported",
			files:   map[string]string{"test.ts": experimentalE + "interface Callable<T = E> { (x: T): T }\nexport declare const f: Callable<string>\n"},
			notWant: []string{"`f` exposes `E`"},
		},
		{
			name:    "replaced generic default instantiation expression",
			files:   map[string]string{"test.ts": experimentalE + "declare function identity<T = E>(x: T): T\nexport const f = identity<string>\n"},
			notWant: []string{"exposes"},
		},
		{
			name:    "eliminated conditional branch",
			files:   map[string]string{"test.ts": experimentalE + "export type Reduced = string extends string ? number : E\n"},
			notWant: []string{"exposes"},
		},
		{
			name:    "nested named surface stays shallow",
			files:   map[string]string{"test.ts": experimentalE + "interface Hidden { value: E }\nexport declare const exposed: { hidden: Hidden }\n"},
			notWant: []string{"exposes"},
		},
		{
			name: "interface instantiated conditional eliminates branch",
			files: singleApiStabilityFile(`interface Holder<T> { value: T extends string ? number : E }
export declare const clean: Holder<string>
export declare const leaky: Holder<E>`),
			want:    []string{"`leaky` exposes `E`"},
			notWant: []string{"`clean` exposes `E`"},
		},
		{
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
			name:    "eliminated conditional alias application",
			files:   singleApiStabilityFile("type Hidden<T> = T extends string ? E : never\nexport declare const clean: Hidden<E>"),
			notWant: []string{"exposes"},
		},
		{
			name:    "compound argument conditional eliminated",
			files:   singleApiStabilityFile("interface Holder<T> { value: T extends string[] ? number : E }\nexport declare const clean: Holder<string[]>"),
			notWant: []string{"exposes"},
		},
		{
			name:    "never distributive conditional eliminated",
			files:   singleApiStabilityFile("interface Holder<T> { value: T extends string ? E : number }\nexport declare const clean: Holder<never>"),
			notWant: []string{"exposes"},
		},
	}
	tests = append(tests, apiStabilityBothOrders("instantiated signature contexts", experimentalE+"declare function make<T>(): (x: T) => T\n",
		"export const clean = make<string>()\n", "export const leaky = make<E>()\n", "`clean` exposes `E`")...)
	tests = append(tests, apiStabilityBothOrders("nested instantiated contexts", experimentalE+"declare function make<T>(): { nested: { fn: (x: T) => T } }\n",
		"export const clean = make<string>()\n", "export const leaky = make<E>()\n", "`clean` exposes `E`")...)
	tests = append(tests, apiStabilityBothOrders("shared raw target", experimentalE+deepAlias+"interface DeepHolder<T> { value: Deep<T> }\n",
		"export declare const clean: DeepHolder<string>\n", "export declare const leaky: DeepHolder<E>\n", "`clean` exposes `E`")...)
	tests = append(tests, apiStabilityBothOrders("shared heritage materialization", experimentalE+"interface Base<T> { value: T }\n",
		"export interface clean extends Base<string> {}\n", "export interface leaky extends Base<E> {}\n", "`clean` exposes `E`")...)

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, test.files)
			runApiStabilityRegressionCase(t, test)
		})
	}
}

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

			exports := ruleCtx.Checker.GetExportsOfModule(ruleCtx.TypeParser.GetSymbolAtLocation(ruleCtx.SourceFile.AsNode()))
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

		exports := ruleCtx.Checker.GetExportsOfModule(ruleCtx.TypeParser.GetSymbolAtLocation(ruleCtx.SourceFile.AsNode()))
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

		exports := ruleCtx.Checker.GetExportsOfModule(ruleCtx.TypeParser.GetSymbolAtLocation(ruleCtx.SourceFile.AsNode()))
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

	exports := ruleCtx.Checker.GetExportsOfModule(ruleCtx.TypeParser.GetSymbolAtLocation(ruleCtx.SourceFile.AsNode()))
	if len(exports) != 1 {
		t.Fatalf("got %d exports, want 1", len(exports))
	}
	symbol := exports[0]

	cold := ruleCtx.TypeParser.ApiStabilityUsedBySymbol(symbol)
	if !containsDependencyName(cold, "E") {
		t.Fatalf("the cold surface should lazily resolve E, got %v", cold.Dependencies())
	}

	_ = apiStabilityCompilerDiagnostics(ruleCtx)
	warm := ruleCtx.TypeParser.ApiStabilityUsedBySymbol(symbol)
	if !containsDependencyName(warm, "E") {
		t.Fatalf("warm surface should expose E, got %v", warm.Dependencies())
	}
	if cold.Minimum() != warm.Minimum() || cold.Dependencies().Len() != warm.Dependencies().Len() {
		t.Fatalf("cold and warm surfaces disagree: cold=%v warm=%v", cold.Dependencies(), warm.Dependencies())
	}
}

func containsDependencyName(used typeparser.ApiStabilityUsed, name string) bool {
	for _, dependency := range used.Dependencies().All() {
		if typeparser.ApiStabilityDependencyName(dependency) == name {
			return true
		}
	}
	return false
}

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

			for _, symbol := range ruleCtx.Checker.GetExportsOfModule(ruleCtx.TypeParser.GetSymbolAtLocation(ruleCtx.SourceFile.AsNode())) {
				if ast.TSGetSymbolName(symbol) != "x" {
					continue
				}
				value := ruleCtx.Checker.GetTypeOfSymbol(symbol)
				for _, property := range ruleCtx.Checker.GetPropertiesOfType(value) {
					if ast.TSGetSymbolName(property) == "value" {
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
				for _, symbol := range ruleCtx.Checker.GetExportsOfModule(ruleCtx.TypeParser.GetSymbolAtLocation(ruleCtx.SourceFile.AsNode())) {
					if used := session.UsedBySymbol(symbol); used.Incomplete() {
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
