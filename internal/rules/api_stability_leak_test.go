package rules

import (
	"context"
	"fmt"
	"slices"
	"sort"
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

func compileApiStabilitySource(t testing.TB, source string) (*rule.Context, *ast.SourceFile, func()) {
	t.Helper()
	return compileApiStabilitySourcePrimed(t, source, true)
}

func compileApiStabilitySourcePrimed(t testing.TB, source string, prime bool) (*rule.Context, *ast.SourceFile, func()) {
	t.Helper()

	testfs := map[string]any{
		"/.src/test.ts": &fstest.MapFile{Data: []byte(source)},
	}
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
		Config:         tsoptions.NewParsedCommandLine(compilerOptions, []tspath.RootedFilePath{"/.src/test.ts"}, nil, "/.src", tspath.CaseSensitive),
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
	return ruleCtx, sf, done
}

func TestApiStabilityLeakExportOrderIsDeterministic(t *testing.T) {
	t.Parallel()

	ruleCtx, done := compileApiStabilityFilesPrimed(t, map[string]string{"test.ts": `/** @stability experimental */
interface E { value: string }
/** @stability experimental */
type P = string
export declare const zeta: E
export declare const alpha: P
export interface Middle { value: E }
export declare const beta: { value: P }
export declare const omega: Middle
`}, true)
	defer done()

	module := checker.Checker_getSymbolOfDeclaration(ruleCtx.Checker, ruleCtx.SourceFile.AsNode())
	if module == nil {
		t.Fatal("module symbol missing")
	}
	all := ruleCtx.Checker.GetExportsOfModule(module)
	if len(all) < 5 {
		t.Fatalf("fixture should export several symbols, got %d", len(all))
	}
	wantNames := make([]string, 0, len(all))
	for _, symbol := range all {
		if symbol != nil {
			wantNames = append(wantNames, ast.TSGetSymbolName(symbol))
		}
	}
	slices.Sort(wantNames)

	var first []string
	for range 64 {
		ordered := sortedApiStabilityExports(ruleCtx, module)
		if len(ordered) != len(all) {
			t.Fatalf("export sorting dropped symbols: got %d, want %d", len(ordered), len(all))
		}
		names := make([]string, 0, len(ordered))
		for index, symbol := range ordered {
			if symbol == nil {
				t.Fatalf("export sorting produced a nil symbol at %d", index)
			}
			names = append(names, ast.TSGetSymbolName(symbol))
		}
		if !slices.IsSorted(names) {
			t.Fatalf("export order is not canonical: %v", names)
		}
		if first == nil {
			first = names
			continue
		}
		if !slices.Equal(first, names) {
			t.Fatalf("export order changed between invocations:\nfirst=%v\nlater=%v", first, names)
		}
	}
	if !slices.Equal(first, wantNames) {
		t.Fatalf("canonical order does not match the sorted export set:\ngot=%v\nwant=%v", first, wantNames)
	}
}

func TestApiStabilityLeakRunsWithoutInstantiation(t *testing.T) {
	t.Parallel()

	aliasChain := map[string]string{
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
		aliasChain[fmt.Sprintf("barrel%d.ts", index)] = fmt.Sprintf("export { Leaky } from %q", "./"+next)
	}

	tests := []apiStabilityRegressionCase{
		{
			name: "internal export filtering",
			files: map[string]string{"test.ts": `/** @stability experimental */
interface E { value: string }
declare const e: E
/** @internal */
export declare const hidden: E
export declare const shown: E
`},
			want:    []string{"`shown` exposes `E`"},
			notWant: []string{"`hidden` exposes"},
		},
		{
			name: "import equals filtering",
			files: map[string]string{"test.ts": `/** @stability experimental */
interface E { value: string }
declare const e: E
namespace Impl {
  /** @internal */
  export interface Hidden { value: E }
  export interface Shown { value: E }
}
export import Hidden = Impl.Hidden
export import Shown = Impl.Shown
`},
			want:    []string{"`Shown` exposes `E`"},
			notWant: []string{"`Hidden` exposes"},
		},
		{
			name: "self-referential surface",
			files: map[string]string{"test.ts": `/** @stability experimental */
interface E { value: string }
export interface Node { next: Node; payload: E }
`},
			want: []string{"`Node` exposes `E`"},
		},
		{
			name:  "40-hop alias chain",
			files: aliasChain,
			want:  []string{"`Leaky` exposes `E`"},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, test.files)
			ruleCtx, done := compileApiStabilityFiles(t, test.files)
			defer done()

			before := ruleCtx.Checker.TotalInstantiationCount
			first := runApiStabilityMessages(t, ruleCtx)
			delta := ruleCtx.Checker.TotalInstantiationCount - before
			second := runApiStabilityMessages(t, ruleCtx)
			delta2 := ruleCtx.Checker.TotalInstantiationCount - before - delta

			if delta != 0 || delta2 != 0 {
				t.Errorf("rule instantiated %d then %d types, want 0", delta, delta2)
			}
			if globals := ruleCtx.Checker.GetGlobalDiagnostics(); len(globals) != 0 {
				t.Errorf("rule produced global diagnostics: %v", globals)
			}
			if !slices.Equal(first, second) {
				t.Errorf("repeated runs disagree:\nfirst=%v\nsecond=%v", first, second)
			}
			for _, want := range test.want {
				if !containsApiStabilityMessage(t, first, want) {
					t.Errorf("missing diagnostic %q in %v", want, first)
				}
			}
			for _, notWant := range test.notWant {
				if containsApiStabilityMessage(t, first, notWant) {
					t.Errorf("unexpected diagnostic %q in %v", notWant, first)
				}
			}
		})
	}
}

func TestApiStabilityLeakSurfaceSharedSnapshotReuse(t *testing.T) {
	t.Parallel()

	files := singleApiStabilityFile(`/** @stability unstable */
export interface ATagged { leak: E }
export interface ZHolder { child: ATagged }
/** @stability unstable */
export interface ZTagged { leak: E }
export interface AHolder { child: ZTagged }
/** @stability stable */
export interface StableBox<T> { value: T }
export interface BoxHolder { box: StableBox<E> }
export declare function makeTagged(): ZTagged
`)
	ruleCtx, done := compileApiStabilityFiles(t, files)
	defer done()

	first := runApiStabilityMessages(t, ruleCtx)
	second := runApiStabilityMessages(t, ruleCtx)
	sortedFirst := slices.Clone(first)
	sortedSecond := slices.Clone(second)
	sort.Strings(sortedFirst)
	sort.Strings(sortedSecond)
	if !slices.Equal(sortedFirst, sortedSecond) {
		t.Fatalf("second run over the shared snapshots disagrees:\nfirst=%v\nsecond=%v", sortedFirst, sortedSecond)
	}

	for _, want := range []string{
		"`ATagged` exposes `E`",
		"`ZTagged` exposes `E`",
		"`ZHolder` exposes `ATagged`, an unstable API.",
		"`AHolder` exposes `ZTagged`, an unstable API.",
		"`BoxHolder` exposes `E`",
		"`makeTagged` exposes `ZTagged`, an unstable API.",
	} {
		if !containsApiStabilityMessage(t, first, want) {
			t.Errorf("missing diagnostic %q in %v", want, first)
		}
	}
	for _, notWant := range []string{
		"`ZHolder` exposes `E`",
		"`AHolder` exposes `E`",
		"`makeTagged` exposes `E`",
		"`ATagged` exposes `ATagged`",
		"`ZTagged` exposes `ZTagged`",
	} {
		if containsApiStabilityMessage(t, first, notWant) {
			t.Errorf("unexpected diagnostic %q in %v", notWant, first)
		}
	}
}
