package typeparser_test

import (
	"context"
	"testing"
	"testing/fstest"

	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/bundled"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/tsoptions"
	"github.com/microsoft/typescript-go/shim/vfs/vfstest"
)

// compileAndGetCheckerAndSourceFile compiles a TypeScript source string and returns the
// checker, type parser, source file, and a done function to release the checker.
func compileAndGetCheckerAndSourceFile(t *testing.T, source string) (*checker.Checker, *typeparser.TypeParser, *ast.SourceFile, func()) {
	t.Helper()

	testfs := map[string]any{
		"/.src/test.ts": &fstest.MapFile{
			Data: []byte(source),
		},
	}

	fs := vfstest.FromMap(testfs, true)
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

	host := compiler.NewCompilerHost("/.src", fs, bundled.LibPath(), nil, nil)
	program := compiler.NewProgram(compiler.ProgramOptions{
		Config: &tsoptions.ParsedCommandLine{
			ParsedConfig: &core.ParsedOptions{
				CompilerOptions: compilerOptions,
				FileNames:       []string{"/.src/test.ts"},
			},
		},
		Host:           host,
		SingleThreaded: core.TSTrue,
	})

	ctx := context.Background()
	c, done := program.GetTypeChecker(ctx)
	sf := program.GetSourceFile("/.src/test.ts")
	if sf == nil {
		done()
		t.Fatal("Failed to get source file")
	}

	return c, typeparser.NewTypeParser(c.Program(), c), sf, done
}

func TestExpectedAndRealTypes_VariableDeclaration(t *testing.T) {
	t.Parallel()

	source := `
const a: number = 42
const b: string = "hello"
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFile(t, source)
	defer done()

	results := tp.ExpectedAndRealTypes(sf)

	// Should find 2 variable declaration sites
	count := 0
	for _, r := range results {
		if r.Node != nil && r.ExpectedType != nil && r.ValueNode != nil && r.RealType != nil {
			count++
		}
	}
	if count < 2 {
		t.Errorf("expected at least 2 assignment sites for variable declarations, got %d (total results: %d)", count, len(results))
	}
}

func TestExpectedAndRealTypes_CallExpression(t *testing.T) {
	t.Parallel()

	source := `
function foo(x: number, y: string): void {}
foo(42, "hello")
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFile(t, source)
	defer done()

	results := tp.ExpectedAndRealTypes(sf)

	// Should find at least 2 call argument sites (for x and y)
	count := 0
	for _, r := range results {
		if r.Node != nil && r.ExpectedType != nil && r.RealType != nil {
			count++
		}
	}
	if count < 2 {
		t.Errorf("expected at least 2 assignment sites for call arguments, got %d (total results: %d)", count, len(results))
	}
}

func TestExpectedAndRealTypes_ObjectLiteralProperty(t *testing.T) {
	t.Parallel()

	source := `
interface Obj { a: number; b: string }
const obj: Obj = { a: 1, b: "hello" }
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFile(t, source)
	defer done()

	results := tp.ExpectedAndRealTypes(sf)

	// Should find sites for the variable declaration and the object literal properties
	if len(results) == 0 {
		t.Error("expected at least one assignment site for object literal, got 0")
	}

	// Verify all results have non-nil types
	for i, r := range results {
		if r.ExpectedType == nil {
			t.Errorf("result[%d]: ExpectedType is nil", i)
		}
		if r.RealType == nil {
			t.Errorf("result[%d]: RealType is nil", i)
		}
	}
}

func TestExpectedAndRealTypes_BinaryAssignment(t *testing.T) {
	t.Parallel()

	source := `
let x: number = 0
x = 42
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFile(t, source)
	defer done()

	results := tp.ExpectedAndRealTypes(sf)

	// Should find at least 2 sites: variable declaration + binary assignment
	if len(results) < 2 {
		t.Errorf("expected at least 2 assignment sites (var decl + binary assign), got %d", len(results))
	}

	// Verify all results have non-nil fields
	for i, r := range results {
		if r.Node == nil {
			t.Errorf("result[%d]: Node is nil", i)
		}
		if r.ExpectedType == nil {
			t.Errorf("result[%d]: ExpectedType is nil", i)
		}
		if r.RealType == nil {
			t.Errorf("result[%d]: RealType is nil", i)
		}
	}
}

func TestExpectedAndRealTypes_ReturnStatement(t *testing.T) {
	t.Parallel()

	source := `
function getNum(): number {
  return 42
}
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFile(t, source)
	defer done()

	results := tp.ExpectedAndRealTypes(sf)

	// Should find at least 1 site for the return statement
	found := false
	for _, r := range results {
		if r.Node != nil && r.Node.Kind == ast.KindReturnStatement {
			found = true
			if r.ExpectedType == nil {
				t.Error("return statement: ExpectedType is nil")
			}
			if r.RealType == nil {
				t.Error("return statement: RealType is nil")
			}
		}
	}
	if !found {
		t.Errorf("expected to find a return statement assignment site, got %d total results", len(results))
	}
}

func TestExpectedAndRealTypes_ArrowFunctionBodyNoTypeParams(t *testing.T) {
	t.Parallel()

	source := `
const fn: () => number = () => 42
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFile(t, source)
	defer done()

	results := tp.ExpectedAndRealTypes(sf)

	// Should find at least 1 site for the variable declaration, and potentially 1 for the arrow body
	if len(results) == 0 {
		t.Error("expected at least 1 assignment site for arrow function body, got 0")
	}

	// Verify all results have valid types
	for i, r := range results {
		if r.ExpectedType == nil {
			t.Errorf("result[%d]: ExpectedType is nil", i)
		}
		if r.RealType == nil {
			t.Errorf("result[%d]: RealType is nil", i)
		}
	}
}

func TestExpectedAndRealTypes_ArrowFunctionBodyWithTypeParams(t *testing.T) {
	t.Parallel()

	source := `
function wrap<T>(fn: () => T): T { return fn() }
const result = wrap(<A extends number>(): A => 42 as any as A)
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFile(t, source)
	defer done()

	results := tp.ExpectedAndRealTypes(sf)

	// Should find at least some assignment sites
	if len(results) == 0 {
		t.Error("expected at least 1 assignment site for arrow function with type params, got 0")
	}
}

func TestExpectedAndRealTypes_SatisfiesExpression(t *testing.T) {
	t.Parallel()

	source := `
const x = 42 satisfies number
const y = "hello" satisfies string
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFile(t, source)
	defer done()

	results := tp.ExpectedAndRealTypes(sf)

	// Should find at least 2 satisfies expression sites plus variable declarations
	satisfiesCount := 0
	for _, r := range results {
		if r.Node != nil && r.ExpectedType != nil && r.RealType != nil {
			satisfiesCount++
		}
	}
	if satisfiesCount < 2 {
		t.Errorf("expected at least 2 assignment sites (satisfies expressions), got %d valid results out of %d total", satisfiesCount, len(results))
	}
}

func TestExpectedAndRealTypes_NilInputs(t *testing.T) {
	t.Parallel()

	// Test nil checker
	results := (*typeparser.TypeParser)(nil).ExpectedAndRealTypes(nil)
	if results != nil {
		t.Errorf("expected nil for nil checker, got %d results", len(results))
	}
}

func TestEffectType_NilInputs(t *testing.T) {
	t.Parallel()

	source := `const x: number = 42`
	_, tp, sf, done := compileAndGetCheckerAndSourceFile(t, source)
	defer done()

	// nil type must not panic
	if result := tp.EffectType(nil, sf.AsNode()); result != nil {
		t.Error("expected nil for nil type")
	}
	// nil checker must not panic
	if result := (*typeparser.TypeParser)(nil).EffectType(nil, sf.AsNode()); result != nil {
		t.Error("expected nil for nil checker")
	}
	// HasEffectTypeId with nil type must not panic
	if tp.HasEffectTypeId(nil, sf.AsNode()) {
		t.Error("expected false for nil type")
	}
}

func TestLayerType_NilInputs(t *testing.T) {
	t.Parallel()

	source := `const x: number = 42`
	_, tp, sf, done := compileAndGetCheckerAndSourceFile(t, source)
	defer done()

	if result := tp.LayerType(nil, sf.AsNode()); result != nil {
		t.Error("expected nil for nil type")
	}
	if result := (*typeparser.TypeParser)(nil).LayerType(nil, sf.AsNode()); result != nil {
		t.Error("expected nil for nil checker")
	}
}

func TestContextTag_NilInputs(t *testing.T) {
	t.Parallel()

	source := `const x: number = 42`
	_, tp, sf, done := compileAndGetCheckerAndSourceFile(t, source)
	defer done()

	if result := tp.ContextTag(nil, sf.AsNode()); result != nil {
		t.Error("expected nil for nil type")
	}
	if result := (*typeparser.TypeParser)(nil).ContextTag(nil, sf.AsNode()); result != nil {
		t.Error("expected nil for nil checker")
	}
}

func TestExpectedAndRealTypes_EmptyFile(t *testing.T) {
	t.Parallel()

	source := ``

	_, tp, sf, done := compileAndGetCheckerAndSourceFile(t, source)
	defer done()

	results := tp.ExpectedAndRealTypes(sf)

	if len(results) != 0 {
		t.Errorf("expected 0 results for empty file, got %d", len(results))
	}
}

func TestExpectedAndRealTypes_AllPatterns(t *testing.T) {
	t.Parallel()

	// A comprehensive source that exercises all 8 patterns in a single file
	source := `
// Pattern 1: Variable declaration with initializer
const a: number = 42

// Pattern 2: Call expression arguments
function takesNum(n: number): void {}
takesNum(10)

// Pattern 3: Object literal property
interface Config { port: number; host: string }
const config: Config = { port: 3000, host: "localhost" }

// Pattern 4: Binary assignment
let x: number = 0
x = 99

// Pattern 5: Return statement
function getVal(): string {
  return "result"
}

// Pattern 6: Arrow function body (no type params)
const arrowFn: () => number = () => 123

// Pattern 7: Arrow function body (with type params)
function identity<T>(fn: () => T): T { return fn() }
const id = identity(<A extends string>(): A => "hi" as any as A)

// Pattern 8: Satisfies expression
const s = "test" satisfies string
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFile(t, source)
	defer done()

	results := tp.ExpectedAndRealTypes(sf)

	// Should find multiple assignment sites
	if len(results) < 8 {
		t.Errorf("expected at least 8 assignment sites from all patterns combined, got %d", len(results))
	}

	// Verify no nil types in results
	for i, r := range results {
		if r.Node == nil {
			t.Errorf("result[%d]: Node is nil", i)
		}
		if r.ExpectedType == nil {
			t.Errorf("result[%d]: ExpectedType is nil", i)
		}
		if r.ValueNode == nil {
			t.Errorf("result[%d]: ValueNode is nil", i)
		}
		if r.RealType == nil {
			t.Errorf("result[%d]: RealType is nil", i)
		}
	}
}

func TestExpectedAndRealTypes_TypeStringVerification(t *testing.T) {
	t.Parallel()

	source := `
const a: number = 42
const b: string = "hello"
`

	c, tp, sf, done := compileAndGetCheckerAndSourceFile(t, source)
	defer done()

	results := tp.ExpectedAndRealTypes(sf)

	// Verify we can use TypeToString on the results (confirms types are valid checker types)
	for i, r := range results {
		if r.ExpectedType != nil {
			str := c.TypeToString(r.ExpectedType)
			if str == "" {
				t.Errorf("result[%d]: TypeToString returned empty for ExpectedType", i)
			}
		}
		if r.RealType != nil {
			str := c.TypeToString(r.RealType)
			if str == "" {
				t.Errorf("result[%d]: TypeToString returned empty for RealType", i)
			}
		}
	}
}
