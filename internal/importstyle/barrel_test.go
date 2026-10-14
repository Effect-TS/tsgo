package importstyle

import (
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/parser"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
)

// fakeFiles serves loaded files as parsed program files and every file,
// loaded or not, through readFile, counting reads per file.
type fakeFiles struct {
	loaded map[tspath.RootedFilePath]*ast.SourceFile
	disk   map[tspath.RootedFilePath]string
	reads  map[tspath.RootedFilePath]int
}

func newFakeFiles(loaded map[string]string, disk map[string]string) *fakeFiles {
	files := &fakeFiles{loaded: map[tspath.RootedFilePath]*ast.SourceFile{}, disk: map[tspath.RootedFilePath]string{}, reads: map[tspath.RootedFilePath]int{}}
	for name, text := range loaded {
		fileName := tspath.RootedFilePath(name)
		files.loaded[fileName] = parser.ParseSourceFile(ast.SourceFileParseOptions{FileName: fileName, PathKey: tspath.PathKey(name)}, text, core.ScriptKindTS)
		files.disk[fileName] = text
	}
	for name, text := range disk {
		files.disk[tspath.RootedFilePath(name)] = text
	}
	return files
}

func (f *fakeFiles) reader() *BarrelReader {
	return NewBarrelReader(
		func(fileName tspath.RootedFilePath) *ast.SourceFile { return f.loaded[fileName] },
		func(fileName tspath.RootedFilePath) (string, bool) {
			f.reads[fileName]++
			text, ok := f.disk[fileName]
			return text, ok
		},
	)
}

func TestIsBarrelFile(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input    string
		expected bool
	}{
		{"/node_modules/effect/dist/index.d.ts", true},
		{"/node_modules/effect/dist/http/index.d.mts", true},
		{"/repo/src/index.ts", true},
		{"/node_modules/effect/dist/Array.d.ts", false},
		{"/node_modules/effect/dist/indexes.d.ts", false},
	}
	for _, tt := range tests {
		if got := IsBarrelFile(tspath.RootedFilePath(tt.input)); got != tt.expected {
			t.Errorf("IsBarrelFile(%q) = %v, want %v", tt.input, got, tt.expected)
		}
	}
}

func TestBarrelReaderFind(t *testing.T) {
	t.Parallel()
	files := newFakeFiles(
		map[string]string{
			"/node_modules/acme/dist/index.d.ts": `export * as Button from "./Button.js"; export * as Forms from "./forms/index.js"; export { helper } from "./helper.js"`,
		},
		map[string]string{
			"/node_modules/acme/dist/forms/index.d.ts":   `export * as TextInput from "./TextInput.ts"`,
			"/node_modules/@scope/kit/lib/index.d.ts":    `export * as Dialog from "./Dialog"`,
			"/node_modules/renamed/dist/index.d.ts":      `export * as Http from "./HttpClient.js"`,
			"/node_modules/parent/dist/index.d.ts":       `export * as Deep from "./nested/Deep.js"`,
			"/node_modules/parent/dist/nested/Deep.d.ts": `export declare const deep: number`,
		},
	)
	tests := []struct {
		name       string
		specifier  string
		moduleFile string
		expected   Barrel
		found      bool
	}{
		{"loaded root barrel", "acme/Button", "/node_modules/acme/dist/Button.d.ts", Barrel{Specifier: "acme", ExportName: "Button"}, true},
		{"nested barrel read from disk", "acme/forms/TextInput", "/node_modules/acme/dist/forms/TextInput.d.ts", Barrel{Specifier: "acme/forms", ExportName: "TextInput"}, true},
		{"scoped package read from disk", "@scope/kit/Dialog", "/node_modules/@scope/kit/lib/Dialog.d.ts", Barrel{Specifier: "@scope/kit", ExportName: "Dialog"}, true},
		{"barrel export name differs from file", "renamed/HttpClient", "/node_modules/renamed/dist/HttpClient.d.ts", Barrel{Specifier: "renamed", ExportName: "Http"}, true},
		{"forwarded from a deeper folder", "parent/nested/Deep", "/node_modules/parent/dist/nested/Deep.d.ts", Barrel{Specifier: "parent", ExportName: "Deep"}, true},
		{"not forwarded by any barrel", "acme/forms/Select", "/node_modules/acme/dist/forms/Select.d.ts", Barrel{}, false},
		{"named export only", "acme/helper", "/node_modules/acme/dist/helper.d.ts", Barrel{}, false},
		{"module without a barrel above it", "acme/private/Secret", "/node_modules/acme/dist/private/Secret.d.ts", Barrel{}, false},
		{"barrel itself", "acme/forms", "/node_modules/acme/dist/forms/index.d.ts", Barrel{}, false},
		{"package root", "acme", "/node_modules/acme/dist/index.d.ts", Barrel{}, false},
		{"relative specifier", "./Button", "/node_modules/acme/dist/Button.d.ts", Barrel{}, false},
	}
	reader := files.reader()
	for _, tt := range tests {
		barrel, found := reader.Find(tt.specifier, tspath.RootedFilePath(tt.moduleFile))
		if found != tt.found || barrel != tt.expected {
			t.Errorf("%s: Find(%q) = %+v, %v, want %+v, %v", tt.name, tt.specifier, barrel, found, tt.expected, tt.found)
		}
	}
}

func TestBarrelReaderReadsEachIndexOnce(t *testing.T) {
	t.Parallel()
	files := newFakeFiles(nil, map[string]string{
		"/node_modules/acme/dist/index.d.ts": `export * as Button from "./Button.js"; export * as Card from "./Card.js"`,
	})
	reader := files.reader()
	for _, module := range []string{"Button", "Card", "Button"} {
		if _, found := reader.Find("acme/"+module, tspath.RootedFilePath("/node_modules/acme/dist/"+module+".d.ts")); !found {
			t.Fatalf("Find(acme/%s) found no barrel", module)
		}
	}
	if got := files.reads["/node_modules/acme/dist/index.d.ts"]; got != 1 {
		t.Errorf("index.d.ts read %d times, want 1", got)
	}
}
