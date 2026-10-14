package importstyle

import (
	"strings"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/parser"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
)

// ProgramFiles is the part of a program barrels are read from.
// *compiler.Program implements it.
type ProgramFiles interface {
	GetSourceFile(fileName tspath.RootedFilePath) *ast.SourceFile
	Host() compiler.CompilerHost
}

// Barrel is the index module a module specifier should be imported through.
type Barrel struct {
	// Specifier imports the barrel, e.g. "effect" or "effect/http".
	Specifier string
	// ExportName is the name of the barrel's `export * as ExportName` that
	// forwards the module.
	ExportName string
}

// BarrelReader finds barrels and caches what each index file forwards.
// It is not safe for concurrent use; keep one per checker or per request.
type BarrelReader struct {
	// programFile returns an already parsed program file, or nil.
	programFile func(fileName tspath.RootedFilePath) *ast.SourceFile
	readFile    func(fileName tspath.RootedFilePath) (string, bool)
	// forwards maps an index file to its forwarded modules, keyed by module
	// path without extension. A nil entry records a missing index file.
	forwards map[tspath.RootedFilePath]map[string]string
}

func NewBarrelReader(
	programFile func(fileName tspath.RootedFilePath) *ast.SourceFile,
	readFile func(fileName tspath.RootedFilePath) (string, bool),
) *BarrelReader {
	return &BarrelReader{programFile: programFile, readFile: readFile, forwards: map[tspath.RootedFilePath]map[string]string{}}
}

// NewProgramBarrelReader reads barrels from the program, falling back to its file system.
func NewProgramBarrelReader(program ProgramFiles) *BarrelReader {
	return NewBarrelReader(program.GetSourceFile, program.Host().FS().ReadFile)
}

var indexExtensions = []string{".d.ts", ".ts", ".tsx", ".d.mts", ".mts", ".d.cts", ".cts", ".js", ".mjs", ".cjs", ".jsx"}

func IsBarrelFile(fileName tspath.RootedFilePath) bool {
	return fileStem(tspath.GetBaseFileName(string(fileName))) == "index"
}

// Find walks up from the file a bare specifier resolves to, one directory and
// one specifier segment at a time, and returns the nearest index module that
// forwards it with `export * as Name`. Index files outside the program are
// read from disk. It assumes the package's public subpaths mirror its
// directory layout below the barrel, as `"./*"` style exports do.
func (r *BarrelReader) Find(specifier string, moduleFileName tspath.RootedFilePath) (Barrel, bool) {
	packageName := PackageName(specifier)
	if r == nil || packageName == "" || moduleFileName == "" || IsBarrelFile(moduleFileName) {
		return Barrel{}, false
	}
	modulePath := tspath.RemoveFileExtension(string(moduleFileName))
	dir := tspath.GetDirectoryPath(string(moduleFileName))
	for spec := specifier; len(spec) > len(packageName); dir = tspath.GetDirectoryPath(dir) {
		spec = spec[:strings.LastIndex(spec, "/")]
		for _, extension := range indexExtensions {
			forwards, exists := r.barrelForwards(tspath.RootedFilePath(tspath.CombinePaths(dir, "index"+extension)))
			if !exists {
				continue
			}
			if name, ok := forwards[modulePath]; ok {
				return Barrel{Specifier: spec, ExportName: name}, true
			}
			break
		}
	}
	return Barrel{}, false
}

func (r *BarrelReader) barrelForwards(fileName tspath.RootedFilePath) (map[string]string, bool) {
	if forwards, cached := r.forwards[fileName]; cached {
		return forwards, forwards != nil
	}
	var forwards map[string]string
	if file := r.parseIndexFile(fileName); file != nil {
		forwards = namespaceForwards(file)
	}
	r.forwards[fileName] = forwards
	return forwards, forwards != nil
}

func (r *BarrelReader) parseIndexFile(fileName tspath.RootedFilePath) *ast.SourceFile {
	if file := r.programFile(fileName); file != nil {
		return file
	}
	text, ok := r.readFile(fileName)
	if !ok {
		return nil
	}
	return parser.ParseSourceFile(ast.SourceFileParseOptions{FileName: fileName}, text, core.GetScriptKindFromFileName(fileName))
}

// namespaceForwards collects the barrel's relative `export * as Name` statements.
// The result is never nil, so an index file without any marks a found barrel.
func namespaceForwards(barrel *ast.SourceFile) map[string]string {
	forwards := map[string]string{}
	dir := tspath.RootedDirectoryPath(tspath.GetDirectoryPath(string(barrel.FileName())))
	for _, statement := range barrel.Statements.Nodes {
		if statement.Kind != ast.KindExportDeclaration {
			continue
		}
		declaration := statement.AsExportDeclaration()
		if declaration.ExportClause == nil || declaration.ExportClause.Kind != ast.KindNamespaceExport ||
			declaration.ModuleSpecifier == nil || declaration.ModuleSpecifier.Kind != ast.KindStringLiteral {
			continue
		}
		relative := declaration.ModuleSpecifier.Text()
		if !strings.HasPrefix(relative, "./") && !strings.HasPrefix(relative, "../") {
			continue
		}
		name := declaration.ExportClause.AsNamespaceExport().Name()
		if name.Kind != ast.KindIdentifier {
			continue
		}
		forwards[tspath.RemoveFileExtension(tspath.GetNormalizedAbsolutePath(relative, dir))] = name.Text()
	}
	return forwards
}

func fileStem(base string) string {
	for _, extension := range declarationExtensions {
		if trimmed, found := strings.CutSuffix(base, extension); found {
			return trimmed
		}
	}
	return base
}
