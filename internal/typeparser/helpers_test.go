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

func compileAndGetCheckerAndSourceFilesInternal(t *testing.T, sources map[string]string) (*checker.Checker, map[string]*ast.SourceFile, func()) {
	t.Helper()

	testfs := make(map[string]any, len(sources))
	fileNames := make([]string, 0, len(sources))
	for path, source := range sources {
		testfs[path] = &fstest.MapFile{Data: []byte(source)}
		fileNames = append(fileNames, path)
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
				FileNames:       fileNames,
			},
		},
		Host:           host,
		SingleThreaded: core.TSTrue,
	})

	ctx := context.Background()
	c, done := program.GetTypeChecker(ctx)
	sourceFiles := make(map[string]*ast.SourceFile, len(fileNames))
	for _, fileName := range fileNames {
		sf := program.GetSourceFile(fileName)
		if sf == nil {
			done()
			t.Fatalf("failed to get source file %s", fileName)
		}
		sourceFiles[fileName] = sf
	}

	return c, sourceFiles, done
}

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

func findIdentifierByText(t *testing.T, sf *ast.SourceFile, text string, occurrence int) *ast.Node {
	t.Helper()

	count := 0
	var found *ast.Node
	var visit func(*ast.Node)
	visit = func(node *ast.Node) {
		if node == nil || found != nil {
			return
		}
		if node.Kind == ast.KindIdentifier {
			if ident := node.AsIdentifier(); ident != nil {
				if ident.Text == text {
					if count == occurrence {
						found = node
						return
					}
					count++
				}
			}
		}
		node.ForEachChild(func(child *ast.Node) bool {
			visit(child)
			return false
		})
	}

	visit(sf.AsNode())
	if found == nil {
		t.Fatalf("identifier %q occurrence %d not found", text, occurrence)
	}
	return found
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
				typ := TypeParser{checker: c}.GetTypeAtLocation(nameNode)
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

func TestGetSymbolIfSameReference_UsesCanonicalExportSymbol(t *testing.T) {
	t.Parallel()

	c, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `export const Foo = 1`)
	defer done()

	localFoo := c.GetSymbolAtLocation(findIdentifierByText(t, sf, "Foo", 0))
	if localFoo == nil {
		t.Fatal("expected local Foo symbol")
	}

	moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
	if moduleSym == nil {
		t.Fatal("expected module symbol")
	}

	exportFoo := c.TryGetMemberInModuleExportsAndProperties("Foo", moduleSym)
	if exportFoo == nil {
		t.Fatal("expected exported Foo symbol")
	}

	if checker.Checker_getSymbolIfSameReference(c, localFoo, exportFoo) == nil {
		t.Fatal("expected local and exported Foo symbols to match")
	}
}

func TestGetSymbolIfSameReference_ResolvesImportAliases(t *testing.T) {
	t.Parallel()

	c, sourceFiles, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/a.ts": `export const Foo = 1`,
		"/.src/b.ts": `import { Foo as Bar } from "./a"
const value = Bar`,
	})
	defer done()

	a := sourceFiles["/.src/a.ts"]
	b := sourceFiles["/.src/b.ts"]
	if a == nil || b == nil {
		t.Fatal("expected both source files")
	}

	moduleSym := checker.Checker_getSymbolOfDeclaration(c, a.AsNode())
	if moduleSym == nil {
		t.Fatal("expected module symbol for a.ts")
	}

	exportFoo := c.TryGetMemberInModuleExportsAndProperties("Foo", moduleSym)
	if exportFoo == nil {
		t.Fatal("expected exported Foo symbol")
	}

	bar := c.GetSymbolAtLocation(findIdentifierByText(t, b, "Bar", 0))
	if bar == nil {
		t.Fatal("expected alias symbol for Bar")
	}

	if checker.Checker_getSymbolIfSameReference(c, exportFoo, bar) == nil {
		t.Fatal("expected exported Foo and imported Bar symbols to match")
	}
	if checker.Checker_getSymbolIfSameReference(c, bar, exportFoo) == nil {
		t.Fatal("expected symbol comparison to be symmetric")
	}
}

func TestGetSymbolIfSameReference_DoesNotMatchDifferentExports(t *testing.T) {
	t.Parallel()

	c, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `export const Foo = 1
export const Bar = 2`)
	defer done()

	foo := c.GetSymbolAtLocation(findIdentifierByText(t, sf, "Foo", 0))
	bar := c.GetSymbolAtLocation(findIdentifierByText(t, sf, "Bar", 0))
	if foo == nil || bar == nil {
		t.Fatal("expected Foo and Bar symbols")
	}

	if checker.Checker_getSymbolIfSameReference(c, foo, bar) != nil {
		t.Fatal("expected different exports not to match")
	}
}
