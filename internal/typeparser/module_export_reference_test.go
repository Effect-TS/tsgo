package typeparser

import (
	"strings"
	"testing"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

const moduleExportReferencePackageJSON = `{
  "name": "cache-fixture",
  "version": "1.0.0",
  "type": "module",
  "exports": { ".": "./index.d.ts" }
}`

func moduleExportReferenceSources() map[string]string {
	return map[string]string{
		"/.src/main.ts": `import { Foo as RootFoo, Bar } from "cache-fixture"
const rootFoo1 = RootFoo
const rootFoo2 = RootFoo
const rootBar = Bar`,
		"/packages/consumer/main.ts": `import { Foo as NestedFoo } from "cache-fixture"
const nestedFoo = NestedFoo`,
		"/node_modules/cache-fixture/package.json": moduleExportReferencePackageJSON,
		"/node_modules/cache-fixture/index.d.ts":   `export { Foo, Bar } from "./source.js"`,
		"/node_modules/cache-fixture/source.d.ts": `export declare const Foo: unique symbol
export declare const Bar: unique symbol`,
		"/packages/consumer/node_modules/cache-fixture/package.json": strings.Replace(moduleExportReferencePackageJSON, "1.0.0", "2.0.0", 1),
		"/packages/consumer/node_modules/cache-fixture/index.d.ts":   `export { Foo } from "./source.js"`,
		"/packages/consumer/node_modules/cache-fixture/source.d.ts":  `export declare const Foo: unique symbol`,
	}
}

func moduleExportReferenceFixture(t testing.TB) (*TypeParser, map[string]*ast.SourceFile, func()) {
	t.Helper()

	_, tp, sourceFiles, done := compileAndGetCheckerAndSourceFilesInternal(t, moduleExportReferenceSources())
	return tp, sourceFiles, done
}

func sourceFileNameMatcher(suffix string, calls *int) func(*TypeParser, *checker.Checker, *ast.SourceFile) bool {
	return func(_ *TypeParser, _ *checker.Checker, sf *ast.SourceFile) bool {
		if calls != nil {
			(*calls)++
		}
		return strings.HasSuffix(sf.FileName(), suffix)
	}
}

func TestIsNodeReferenceToModuleExport_AliasesAndReexports(t *testing.T) {
	t.Parallel()

	tp, sourceFiles, done := moduleExportReferenceFixture(t)
	defer done()

	desc := newPackageSourceFileDescriptor("cache-fixture", sourceFileNameMatcher("/source.d.ts", nil))
	main := sourceFiles["/.src/main.ts"]
	rootFoo := findIdentifierByText(t, main, "RootFoo", 1)
	rootBar := findIdentifierByText(t, main, "Bar", 1)

	if !tp.IsNodeReferenceToModuleExport(rootFoo, desc, "Foo") {
		t.Fatal("expected aliased Foo reference through a reexport to match")
	}
	if tp.IsNodeReferenceToModuleExport(rootFoo, desc, "Bar") {
		t.Fatal("expected Foo reference not to match the Bar export")
	}
	if !tp.IsNodeReferenceToModuleExport(rootBar, desc, "Bar") {
		t.Fatal("expected Bar reference through a reexport to match")
	}
}

func TestIsNodeReferenceToModuleExport_DuplicatePackageVersions(t *testing.T) {
	t.Parallel()

	tp, sourceFiles, done := moduleExportReferenceFixture(t)
	defer done()

	desc := newPackageSourceFileDescriptor("cache-fixture", nil)
	rootFoo := findIdentifierByText(t, sourceFiles["/.src/main.ts"], "RootFoo", 1)
	nestedFoo := findIdentifierByText(t, sourceFiles["/packages/consumer/main.ts"], "NestedFoo", 1)

	if !tp.IsNodeReferenceToModuleExport(rootFoo, desc, "Foo") {
		t.Fatal("expected Foo from the root package version to match")
	}
	if !tp.IsNodeReferenceToModuleExport(nestedFoo, desc, "Foo") {
		t.Fatal("expected Foo from the nested package version to match independently")
	}
	if tp.ReferenceSymbolAtNode(rootFoo) == tp.ReferenceSymbolAtNode(nestedFoo) {
		t.Fatal("expected duplicate package versions to resolve to distinct symbols")
	}
}

func TestIsNodeReferenceToModuleExport_CachesBySymbolDescriptorAndMember(t *testing.T) {
	t.Parallel()

	tp, sourceFiles, done := moduleExportReferenceFixture(t)
	defer done()

	main := sourceFiles["/.src/main.ts"]
	rootFoo1 := findIdentifierByText(t, main, "RootFoo", 1)
	rootFoo2 := findIdentifierByText(t, main, "RootFoo", 2)
	calls := 0
	matchingDesc := newPackageSourceFileDescriptor("cache-fixture", sourceFileNameMatcher("/source.d.ts", &calls))

	if !tp.IsNodeReferenceToModuleExport(rootFoo1, matchingDesc, "Foo") {
		t.Fatal("expected first Foo reference to match")
	}
	firstCallCount := calls
	if !tp.IsNodeReferenceToModuleExport(rootFoo2, matchingDesc, "Foo") {
		t.Fatal("expected second Foo reference to match")
	}
	if calls != firstCallCount {
		t.Fatalf("expected resolved-symbol cache hit, matcher calls increased from %d to %d", firstCallCount, calls)
	}

	nonMatchingDesc := newPackageSourceFileDescriptor("cache-fixture", sourceFileNameMatcher("/other.d.ts", nil))
	if tp.IsNodeReferenceToModuleExport(rootFoo1, nonMatchingDesc, "Foo") {
		t.Fatal("expected a different source descriptor not to reuse the cached match")
	}
	if tp.IsNodeReferenceToModuleExport(rootFoo1, matchingDesc, "Bar") {
		t.Fatal("expected a different member not to reuse the cached Foo match")
	}
}

func BenchmarkIsNodeReferenceToModuleExport(b *testing.B) {
	tp, sourceFiles, done := moduleExportReferenceFixture(b)
	defer done()

	desc := newPackageSourceFileDescriptor("cache-fixture", sourceFileNameMatcher("/source.d.ts", nil))
	rootFoo := findIdentifierByText(b, sourceFiles["/.src/main.ts"], "RootFoo", 1)

	b.ResetTimer()
	for b.Loop() {
		if !tp.IsNodeReferenceToModuleExport(rootFoo, desc, "Foo") {
			b.Fatal("expected Foo reference to match")
		}
	}
}
