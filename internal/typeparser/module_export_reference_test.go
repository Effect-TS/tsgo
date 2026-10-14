package typeparser

import (
	"strings"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

const moduleExportReferencePackageJSON = `{
  "name": "cache-fixture",
  "version": "1.0.0",
  "type": "module",
  "exports": { ".": "./index.d.ts" }
}`

const (
	testDescriptorSourceFile = moduleDescriptorCount + iota
	testDescriptorAnyFile
	testDescriptorOtherFile
)

func moduleExportReferenceSources() map[string]string {
	return map[string]string{
		"/.src/main.ts": `import { Foo as RootFoo, Bar } from "cache-fixture"
const rootFoo1 = RootFoo
const rootFoo2 = RootFoo
const rootBar = Bar
const rootFooAlias = RootFoo
const rootFooAlias2 = rootFooAlias
const rootFooAliasUse = rootFooAlias2
let mutableFooAlias = RootFoo
mutableFooAlias = Bar
const mutableFooAliasUse = mutableFooAlias`,
		"/packages/consumer/main.ts": `import { Foo as NestedFoo } from "cache-fixture"
const nestedFoo = NestedFoo`,
		"/node_modules/cache-fixture/package.json": moduleExportReferencePackageJSON,
		"/.src/renamed.ts": `import { Base, Renamed } from "cache-fixture"
const baseUse = Base
const renamedUse = Renamed`,
		"/.src/cjs.ts": `import api = require("cjs-fixture")
const runUse = api.run`,
		"/node_modules/cache-fixture/index.d.ts": `export { Foo, Bar } from "./source.js"
export { Base, Renamed } from "./renamed.js"`,
		"/node_modules/cache-fixture/source.d.ts": `export declare const Foo: unique symbol
export declare const Bar: unique symbol`,
		"/node_modules/cache-fixture/renamed.d.ts": `declare const Base: unique symbol
export { Base, Base as Renamed }`,
		"/node_modules/cjs-fixture/package.json": `{ "name": "cjs-fixture", "version": "1.0.0", "types": "./index.d.ts" }`,
		"/node_modules/cjs-fixture/index.d.ts": `declare const api: { run(): void; stop(): void }
export = api`,
		"/packages/consumer/node_modules/cache-fixture/package.json": strings.Replace(moduleExportReferencePackageJSON, "1.0.0", "2.0.0", 1),
		"/packages/consumer/node_modules/cache-fixture/index.d.ts":   `export { Foo } from "./source.js"`,
		"/packages/consumer/node_modules/cache-fixture/source.d.ts":  `export declare const Foo: unique symbol`,
	}
}

func TestIsNodeReferenceToModuleExport_ConstantAliases(t *testing.T) {
	t.Parallel()

	tp, sourceFiles, done := moduleExportReferenceFixture(t)
	defer done()

	desc := newPackageSourceFileDescriptor(testDescriptorSourceFile, "cache-fixture", sourceFileNameMatcher("/source.d.ts", nil))
	main := sourceFiles["/.src/main.ts"]
	constantAliasDeclaration := findIdentifierByText(t, main, "rootFooAlias2", 0)
	constantAlias := findIdentifierByText(t, main, "rootFooAlias2", 1)
	mutableAlias := findIdentifierByText(t, main, "mutableFooAlias", 2)

	if tp.IsNodeReferenceToModuleExport(constantAliasDeclaration, desc, "Foo") {
		t.Fatal("expected a constant alias declaration name not to match the Foo export")
	}
	if !tp.IsNodeReferenceToModuleExport(constantAlias, desc, "Foo") {
		t.Fatal("expected a constant alias chain to match the Foo export")
	}
	if tp.IsNodeReferenceToModuleExport(mutableAlias, desc, "Foo") {
		t.Fatal("expected a mutable alias not to match the Foo export")
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
		return strings.HasSuffix(string(sf.FileName()), suffix)
	}
}

func TestIsNodeReferenceToModuleExport_AliasesAndReexports(t *testing.T) {
	t.Parallel()

	tp, sourceFiles, done := moduleExportReferenceFixture(t)
	defer done()

	desc := newPackageSourceFileDescriptor(testDescriptorSourceFile, "cache-fixture", sourceFileNameMatcher("/source.d.ts", nil))
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

	desc := newPackageSourceFileDescriptor(testDescriptorAnyFile, "cache-fixture", nil)
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
	matchingDesc := newPackageSourceFileDescriptor(testDescriptorSourceFile, "cache-fixture", sourceFileNameMatcher("/source.d.ts", &calls))

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

	nonMatchingDesc := newPackageSourceFileDescriptor(testDescriptorOtherFile, "cache-fixture", sourceFileNameMatcher("/other.d.ts", nil))
	if tp.IsNodeReferenceToModuleExport(rootFoo1, nonMatchingDesc, "Foo") {
		t.Fatal("expected a different source descriptor not to reuse the cached match")
	}
	if tp.IsNodeReferenceToModuleExport(rootFoo1, matchingDesc, "Bar") {
		t.Fatal("expected a different member not to reuse the cached Foo match")
	}
}

func TestIsNodeReferenceToModuleExport_SymbolExportedUnderTwoNames(t *testing.T) {
	t.Parallel()

	tp, sourceFiles, done := moduleExportReferenceFixture(t)
	defer done()

	desc := newPackageSourceFileDescriptor(testDescriptorSourceFile, "cache-fixture", sourceFileNameMatcher("/renamed.d.ts", nil))
	renamed := sourceFiles["/.src/renamed.ts"]
	for _, node := range []*ast.Node{findIdentifierByText(t, renamed, "Base", 1), findIdentifierByText(t, renamed, "Renamed", 1)} {
		for _, member := range []string{"Base", "Renamed"} {
			if !tp.IsNodeReferenceToModuleExport(node, desc, member) {
				t.Fatalf("expected %s to match the %s export of the same declaration", node.Text(), member)
			}
		}
		if tp.IsNodeReferenceToModuleExport(node, desc, "Foo") {
			t.Fatalf("expected %s not to match the Foo export", node.Text())
		}
	}
}

func TestIsNodeReferenceToModuleExport_ExportEqualsMembers(t *testing.T) {
	t.Parallel()

	tp, sourceFiles, done := moduleExportReferenceFixture(t)
	defer done()

	desc := newPackageSourceFileDescriptor(testDescriptorAnyFile, "cjs-fixture", nil)
	run := findIdentifierByText(t, sourceFiles["/.src/cjs.ts"], "run", 0)

	if !tp.IsNodeReferenceToModuleExport(run, desc, "run") {
		t.Fatal("expected a member of an export= value to match its name")
	}
	if tp.IsNodeReferenceToModuleExport(run, desc, "stop") {
		t.Fatal("expected run not to match the stop member")
	}
}

func BenchmarkIsNodeReferenceToModuleExport(b *testing.B) {
	tp, sourceFiles, done := moduleExportReferenceFixture(b)
	defer done()

	desc := newPackageSourceFileDescriptor(testDescriptorSourceFile, "cache-fixture", sourceFileNameMatcher("/source.d.ts", nil))
	rootFoo := findIdentifierByText(b, sourceFiles["/.src/main.ts"], "RootFoo", 1)

	b.ResetTimer()
	for b.Loop() {
		if !tp.IsNodeReferenceToModuleExport(rootFoo, desc, "Foo") {
			b.Fatal("expected Foo reference to match")
		}
	}
}
