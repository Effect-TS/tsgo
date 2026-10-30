package typeparser

import (
	"context"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"strings"
	"testing"
	"testing/fstest"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/bundled"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

func compileAndGetCheckerAndSourceFilesInternal(t testing.TB, sources map[string]string) (*checker.Checker, *TypeParser, map[string]*ast.SourceFile, func()) {
	t.Helper()

	testfs := make(map[string]any, len(sources))
	fileNames := make([]tspath.RootedFilePath, 0, len(sources))
	for path, source := range sources {
		testfs[path] = &fstest.MapFile{Data: []byte(source)}
		if !strings.Contains(path, "/node_modules/") && strings.HasSuffix(path, ".ts") {
			fileNames = append(fileNames, tspath.RootedFilePath(path))
		}
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
		Config:         tsoptions.NewParsedCommandLine(compilerOptions, fileNames, nil, "/", tspath.CaseSensitive),
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
		sourceFiles[string(fileName)] = sf
	}

	return c, NewTypeParser(c.Program(), c), sourceFiles, done
}

func compileAndGetCheckerAndSourceFileInternal(t *testing.T, source string) (*checker.Checker, *TypeParser, *ast.SourceFile, func()) {
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

	return c, NewTypeParser(c.Program(), c), sf, done
}

func compileAndGetCheckerAndSourceFileWithEffectV4Internal(t *testing.T, source string) (*checker.Checker, *TypeParser, *ast.SourceFile, func()) {
	return compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, bundledeffect.EffectV4, source)
}

func compileAndGetCheckerAndSourceFileWithEffectV3Internal(t *testing.T, source string) (*checker.Checker, *TypeParser, *ast.SourceFile, func()) {
	return compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, bundledeffect.EffectV3, source)
}

func compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t *testing.T, version bundledeffect.EffectVersion, source string) (*checker.Checker, *TypeParser, *ast.SourceFile, func()) {
	t.Helper()

	testfs := map[string]any{
		"/.src/test.ts": &fstest.MapFile{
			Data: []byte(source),
		},
	}
	if err := bundledeffect.MountEffect(version, testfs); err != nil {
		t.Fatalf("failed to mount effect %s: %v", version, err)
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

	return c, NewTypeParser(c.Program(), c), sf, done
}

func findIdentifierByText(t testing.TB, sf *ast.SourceFile, text string, occurrence int) *ast.Node {
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

func getFirstVariableDeclarationType(t *testing.T, tp *TypeParser, sf *ast.SourceFile) (*checker.Type, *ast.Node) {
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
				typ := tp.GetTypeAtLocation(nameNode)
				return typ, node
			}
		}
		node.ForEachChild(enqueueChild)
	}

	t.Fatal("No variable declaration found in source file")
	return nil, nil
}

func TestExtractVarianceTypesRejectGenericSignatures(t *testing.T) {
	t.Parallel()

	c, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `declare const test: {
  covariant: () => string
  genericCovariant: <T>() => T
  contravariant: (_: string) => void
  genericContravariant: <T>(_: T) => void
  invariant: (_: string) => string
  genericInvariant: <T>(_: T) => T
}`)
	defer done()

	typ, _ := getFirstVariableDeclarationType(t, tp, sf)

	for _, tc := range []struct {
		prop    string
		extract func(*checker.Type, string) *checker.Type
		want    string
	}{
		{"covariant", tp.extractCovariantType, "string"},
		{"genericCovariant", tp.extractCovariantType, ""},
		{"contravariant", tp.extractContravariantType, "string"},
		{"genericContravariant", tp.extractContravariantType, ""},
		{"invariant", tp.extractInvariantType, "string"},
		{"genericInvariant", tp.extractInvariantType, ""},
	} {
		result := tc.extract(typ, tc.prop)
		switch {
		case tc.want == "" && result != nil:
			t.Errorf("%s: expected nil for generic signature, got %s", tc.prop, c.TypeToString(result))
		case tc.want != "" && result == nil:
			t.Errorf("%s: expected %q, got nil", tc.prop, tc.want)
		case tc.want != "" && c.TypeToString(result) != tc.want:
			t.Errorf("%s: expected %q, got %q", tc.prop, tc.want, c.TypeToString(result))
		}
	}
}

func TestGetSymbolIfSameReferenceWithinModule(t *testing.T) {
	t.Parallel()

	c, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `export const Foo = 1
export const Bar = 2`)
	defer done()

	localFoo := tp.GetSymbolAtLocation(findIdentifierByText(t, sf, "Foo", 0))
	bar := tp.GetSymbolAtLocation(findIdentifierByText(t, sf, "Bar", 0))
	if localFoo == nil || bar == nil {
		t.Fatal("expected Foo and Bar symbols")
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
	if checker.Checker_getSymbolIfSameReference(c, localFoo, bar) != nil {
		t.Fatal("expected different exports not to match")
	}
}

func TestGetSymbolIfSameReference_ResolvesImportAliases(t *testing.T) {
	t.Parallel()

	c, tp, sourceFiles, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/a.ts": `export const Foo = 1`,
		"/.src/b.ts": `import { Foo as Bar } from "./a"
const value = Bar`,
	})
	defer done()

	a := sourceFiles["/.src/a.ts"]
	b := sourceFiles["/.src/b.ts"]
	if a == nil || b == nil {
		t.Fatal("expected both source files")
		return
	}

	moduleSym := checker.Checker_getSymbolOfDeclaration(c, a.AsNode())
	if moduleSym == nil {
		t.Fatal("expected module symbol for a.ts")
	}

	exportFoo := c.TryGetMemberInModuleExportsAndProperties("Foo", moduleSym)
	if exportFoo == nil {
		t.Fatal("expected exported Foo symbol")
	}

	bar := tp.GetSymbolAtLocation(findIdentifierByText(t, b, "Bar", 0))
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
