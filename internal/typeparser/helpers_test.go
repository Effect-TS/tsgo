package typeparser

import (
	"context"
	"testing"
	"testing/fstest"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/bundled"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/tsoptions"
	"github.com/microsoft/typescript-go/shim/vfs/vfstest"
)

// compileAndGetCheckerAndSourceFileInternal is a copy of compileAndGetCheckerAndSourceFile
// for use in internal (package typeparser) tests that need access to unexported helpers.
func compileAndGetCheckerAndSourceFileInternal(t *testing.T, source string) (*checker.Checker, *ast.SourceFile, func()) {
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

	return c, sf, done
}

// getFirstVariableDeclarationType finds the first variable declaration in the source file
// and returns its type and the declaration node (for use as atLocation).
func getFirstVariableDeclarationType(t *testing.T, c *checker.Checker, sf *ast.SourceFile) (*checker.Type, *ast.Node) {
	t.Helper()

	queue := []*ast.Node{sf.AsNode()}
	enqueueChild := func(child *ast.Node) bool {
		queue = append(queue, child)
		return false
	}
	for len(queue) > 0 {
		node := queue[0]
		queue = queue[1:]
		if node == nil {
			continue
		}
		if node.Kind == ast.KindVariableDeclaration {
			nameNode := node.AsVariableDeclaration().Name()
			if nameNode != nil {
				typ := GetTypeAtLocation(c, nameNode)
				return typ, node
			}
		}
		node.ForEachChild(enqueueChild)
	}

	t.Fatal("No variable declaration found in source file")
	return nil, nil
}

func TestExtractCovariantType_RejectsGenericSignature(t *testing.T) {
	t.Parallel()

	source := `declare const test: { prop: <T>() => T }`

	c, sf, done := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done()

	typ, decl := getFirstVariableDeclarationType(t, c, sf)

	result := extractCovariantType(c, typ, decl, "prop")
	if result != nil {
		t.Errorf("expected nil for generic covariant signature, got %s", c.TypeToString(result))
	}
}

func TestExtractContravariantType_RejectsGenericSignature(t *testing.T) {
	t.Parallel()

	source := `declare const test: { prop: <T>(_: T) => void }`

	c, sf, done := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done()

	typ, decl := getFirstVariableDeclarationType(t, c, sf)

	result := extractContravariantType(c, typ, decl, "prop")
	if result != nil {
		t.Errorf("expected nil for generic contravariant signature, got %s", c.TypeToString(result))
	}
}

func TestExtractCovariantType_AcceptsNonGenericSignature(t *testing.T) {
	t.Parallel()

	source := `declare const test: { prop: () => string }`

	c, sf, done := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done()

	typ, decl := getFirstVariableDeclarationType(t, c, sf)

	result := extractCovariantType(c, typ, decl, "prop")
	if result == nil {
		t.Fatal("expected non-nil result for non-generic covariant signature")
	}

	resultStr := c.TypeToString(result)
	if resultStr != "string" {
		t.Errorf("expected return type 'string', got %q", resultStr)
	}
}

func TestExtractInvariantType_RejectsGenericSignature(t *testing.T) {
	t.Parallel()

	source := `declare const test: { prop: <T>(_: T) => T }`

	c, sf, done := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done()

	typ, decl := getFirstVariableDeclarationType(t, c, sf)

	result := extractInvariantType(c, typ, decl, "prop")
	if result != nil {
		t.Errorf("expected nil for generic invariant signature, got %s", c.TypeToString(result))
	}
}

func TestExtractInvariantType_AcceptsNonGenericSignature(t *testing.T) {
	t.Parallel()

	source := `declare const test: { prop: (_: string) => string }`

	c, sf, done := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done()

	typ, decl := getFirstVariableDeclarationType(t, c, sf)

	result := extractInvariantType(c, typ, decl, "prop")
	if result == nil {
		t.Fatal("expected non-nil result for non-generic invariant signature")
	}

	resultStr := c.TypeToString(result)
	if resultStr != "string" {
		t.Errorf("expected return type 'string', got %q", resultStr)
	}
}

func TestExtractContravariantType_AcceptsNonGenericSignature(t *testing.T) {
	t.Parallel()

	source := `declare const test: { prop: (_: string) => void }`

	c, sf, done := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done()

	typ, decl := getFirstVariableDeclarationType(t, c, sf)

	result := extractContravariantType(c, typ, decl, "prop")
	if result == nil {
		t.Fatal("expected non-nil result for non-generic contravariant signature")
	}

	resultStr := c.TypeToString(result)
	if resultStr != "string" {
		t.Errorf("expected parameter type 'string', got %q", resultStr)
	}
}
