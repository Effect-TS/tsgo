package typeparser_test

import (
	"context"
	"fmt"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"slices"
	"strings"
	"testing"
	"testing/fstest"

	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/bundled"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

func compileAndGetCheckerAndSourceFile(t *testing.T, source string) (*checker.Checker, *typeparser.TypeParser, *ast.SourceFile, func()) {
	t.Helper()

	testfs := map[string]any{
		"/.src/test.ts": &fstest.MapFile{
			Data: []byte(source),
		},
	}

	fs := vfstest.FromMap(testfs, tspath.CaseSensitive)
	fs = bundled.WrapFS(fs)

	compilerOptions := &core.CompilerOptions{
		NewLine:             core.NewLineKindLF,
		SkipDefaultLibCheck: core.TSTrue,
		NoErrorTruncation:   core.TSTrue,
		Target:              core.ScriptTargetESNext,
		Module:              core.ModuleKindNodeNext,
		ModuleResolution:    core.ModuleResolutionKindNodeNext,
		Strict:              core.TSTrue,
	}

	host := compiler.NewCompilerHost(fs, bundled.LibPath(), nil, nil, nil)
	program := compiler.NewProgram(compiler.ProgramOptions{
		Config:         tsoptions.NewParsedCommandLine(compilerOptions, []tspath.RootedFilePath{"/.src/test.ts"}, nil, "/", tspath.CaseSensitive),
		Host:           host,
		SingleThreaded: core.TSTrue,
	})

	ctx := context.Background()
	c, done := program.GetTypeChecker(ctx)
	sf := program.GetSourceFile(tspath.RootedFilePath("/.src/test.ts"))
	if sf == nil {
		done()
		t.Fatal("Failed to get source file")
	}

	return c, typeparser.NewTypeParser(c.Program(), c), sf, done
}

func TestExpectedAndRealTypes(t *testing.T) {
	t.Parallel()

	source := `
const a: number = 42

function takesNum(n: number): void {}
takesNum(10)

interface Config { port: number; host: string }
const config: Config = { port: 3000, host: "localhost" }

let x: number = 0
x = 99

function getVal(): string {
  return "result"
}

const arrowFn: () => number = () => 123

function identity<T>(fn: () => T): T { return fn() }
const id = identity(<A extends string>(): A => "hi" as any as A)

const s = "test" satisfies string
`

	c, tp, sf, done := compileAndGetCheckerAndSourceFile(t, source)
	defer done()

	var got []string
	for _, r := range tp.ExpectedAndRealTypes(sf).All() {
		got = append(got, fmt.Sprintf("%s | %s | %s | %s",
			r.Node().Kind,
			strings.TrimSpace(scanner.GetTextOfNode(r.ValueNode())),
			c.TypeToString(r.ExpectedType()),
			c.TypeToString(r.RealType()),
		))
	}
	want := []string{
		`KindNumericLiteral | 10 | number | 10`,
		`KindIdentifier | 99 | number | 99`,
		`KindIdentifier | 42 | number | 42`,
		`KindIdentifier | { port: 3000, host: "localhost" } | Config | { port: number; host: string; }`,
		`KindIdentifier | 0 | number | 0`,
		`KindReturnStatement | return "result" | string | "result"`,
		`KindIdentifier | () => 123 | () => number | () => number`,
		`KindReturnStatement | return fn() | T | T`,
		`KindIdentifier | identity(<A extends string>(): A => "hi" as any as A) | string | string`,
		`KindIdentifier | "test" satisfies string | "test" | "test"`,
		`KindNumericLiteral | 123 | number | 123`,
		`KindArrowFunction | <A extends string>(): A => "hi" as any as A | () => string | <A extends string>() => A`,
		`KindStringLiteral | "test" | string | "test"`,
		`KindAsExpression | "hi" as any as A | A | A`,
		`KindIdentifier | port | number | number`,
		`KindIdentifier | host | string | string`,
	}
	if !slices.Equal(got, want) {
		t.Errorf("ExpectedAndRealTypes =\n%s\nwant\n%s", strings.Join(got, "\n"), strings.Join(want, "\n"))
	}
}

func TestExpectedAndRealTypes_EmptyFile(t *testing.T) {
	t.Parallel()

	_, tp, sf, done := compileAndGetCheckerAndSourceFile(t, ``)
	defer done()

	if results := tp.ExpectedAndRealTypes(sf); results.Len() != 0 {
		t.Errorf("expected 0 results for empty file, got %d", results.Len())
	}
}

func TestTypeParser_NilInputs(t *testing.T) {
	t.Parallel()

	_, tp, _, done := compileAndGetCheckerAndSourceFile(t, `const x: number = 42`)
	defer done()

	var nilTp *typeparser.TypeParser
	if results := nilTp.ExpectedAndRealTypes(nil); !results.IsNil() {
		t.Errorf("ExpectedAndRealTypes: expected nil for nil checker, got %d results", results.Len())
	}
	for _, parser := range []*typeparser.TypeParser{tp, nilTp} {
		if parser.EffectType(nil) != nil {
			t.Error("EffectType: expected nil for nil input")
		}
		if parser.LayerType(nil) != nil {
			t.Error("LayerType: expected nil for nil input")
		}
		if parser.ContextTag(nil) != nil {
			t.Error("ContextTag: expected nil for nil input")
		}
	}
	if tp.HasEffectTypeId(nil) {
		t.Error("HasEffectTypeId: expected false for nil type")
	}
}
