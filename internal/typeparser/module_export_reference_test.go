package typeparser

import (
	"context"
	"slices"
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
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
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
	testDescriptorRenamedFile
	testDescriptorCjsFile
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

func TestIsNodeReferenceToModuleExport(t *testing.T) {
	t.Parallel()

	tp, sourceFiles, done := moduleExportReferenceFixture(t)
	defer done()

	sourceDesc := newPackageSourceFileDescriptor(testDescriptorSourceFile, "cache-fixture", sourceFileNameMatcher("/source.d.ts", nil))
	anyDesc := newPackageSourceFileDescriptor(testDescriptorAnyFile, "cache-fixture", nil)
	renamedDesc := newPackageSourceFileDescriptor(testDescriptorRenamedFile, "cache-fixture", sourceFileNameMatcher("/renamed.d.ts", nil))
	cjsDesc := newPackageSourceFileDescriptor(testDescriptorCjsFile, "cjs-fixture", nil)
	for _, tc := range []struct {
		name       string
		file       string
		identifier string
		occurrence int
		desc       packageSourceFileDescriptor
		member     string
		want       bool
	}{
		{"constant alias declaration name", "/.src/main.ts", "rootFooAlias2", 0, sourceDesc, "Foo", false},
		{"constant alias chain", "/.src/main.ts", "rootFooAlias2", 1, sourceDesc, "Foo", true},
		{"mutable alias", "/.src/main.ts", "mutableFooAlias", 2, sourceDesc, "Foo", false},
		{"aliased reexport", "/.src/main.ts", "RootFoo", 1, sourceDesc, "Foo", true},
		{"aliased reexport other member", "/.src/main.ts", "RootFoo", 1, sourceDesc, "Bar", false},
		{"reexport", "/.src/main.ts", "Bar", 1, sourceDesc, "Bar", true},
		{"root package version", "/.src/main.ts", "RootFoo", 1, anyDesc, "Foo", true},
		{"nested package version", "/packages/consumer/main.ts", "NestedFoo", 1, anyDesc, "Foo", true},
		{"Base as Base", "/.src/renamed.ts", "Base", 1, renamedDesc, "Base", true},
		{"Base as Renamed", "/.src/renamed.ts", "Base", 1, renamedDesc, "Renamed", true},
		{"Base as Foo", "/.src/renamed.ts", "Base", 1, renamedDesc, "Foo", false},
		{"Renamed as Base", "/.src/renamed.ts", "Renamed", 1, renamedDesc, "Base", true},
		{"Renamed as Renamed", "/.src/renamed.ts", "Renamed", 1, renamedDesc, "Renamed", true},
		{"Renamed as Foo", "/.src/renamed.ts", "Renamed", 1, renamedDesc, "Foo", false},
		{"export= member", "/.src/cjs.ts", "run", 0, cjsDesc, "run", true},
		{"export= other member", "/.src/cjs.ts", "run", 0, cjsDesc, "stop", false},
	} {
		node := findIdentifierByText(t, sourceFiles[tc.file], tc.identifier, tc.occurrence)
		if got := tp.IsNodeReferenceToModuleExport(node, tc.desc, tc.member); got != tc.want {
			t.Errorf("%s: got %v, want %v", tc.name, got, tc.want)
		}
	}
	rootFoo := findIdentifierByText(t, sourceFiles["/.src/main.ts"], "RootFoo", 1)
	nestedFoo := findIdentifierByText(t, sourceFiles["/packages/consumer/main.ts"], "NestedFoo", 1)
	if tp.ReferenceSymbolAtNode(rootFoo) == tp.ReferenceSymbolAtNode(nestedFoo) {
		t.Fatal("expected duplicate package versions to resolve to distinct symbols")
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

func compileFixturesWithJS(t testing.TB, version bundledeffect.EffectVersion, sources map[string]string) (*TypeParser, []*ast.SourceFile, func()) {
	t.Helper()

	testfs := make(map[string]any, len(sources))
	if version != "" {
		if err := bundledeffect.MountEffect(version, testfs); err != nil {
			t.Fatalf("failed to mount effect %s: %v", version, err)
		}
	}
	var fileNames []tspath.RootedFilePath
	for path, source := range sources {
		testfs[path] = &fstest.MapFile{Data: []byte(source)}
		if !strings.Contains(path, "/node_modules/") &&
			(strings.HasSuffix(path, ".ts") || strings.HasSuffix(path, ".tsx") || strings.HasSuffix(path, ".js")) {
			fileNames = append(fileNames, tspath.RootedFilePath(path))
		}
	}
	slices.Sort(fileNames)

	compilerOptions := &core.CompilerOptions{
		NewLine:             core.NewLineKindLF,
		SkipDefaultLibCheck: core.TSTrue,
		NoErrorTruncation:   core.TSTrue,
		Target:              core.ScriptTargetESNext,
		Module:              core.ModuleKindNodeNext,
		ModuleResolution:    core.ModuleResolutionKindNodeNext,
		Strict:              core.TSTrue,
		AllowJs:             core.TSTrue,
		CheckJs:             core.TSTrue,
		Jsx:                 core.JsxEmitPreserve,
	}
	fs := bundled.WrapFS(vfstest.FromMap(testfs, tspath.CaseSensitive))
	host := compiler.NewCompilerHost(fs, bundled.LibPath(), nil, nil, nil)
	program := compiler.NewProgram(compiler.ProgramOptions{
		Config:         tsoptions.NewParsedCommandLine(compilerOptions, fileNames, nil, "/", tspath.CaseSensitive),
		Host:           host,
		SingleThreaded: core.TSTrue,
	})

	c, done := program.GetTypeChecker(context.Background())
	sourceFiles := make([]*ast.SourceFile, 0, len(fileNames))
	for _, fileName := range fileNames {
		sf := program.GetSourceFile(fileName)
		if sf == nil {
			done()
			t.Fatalf("failed to get source file %s", fileName)
		}
		sourceFiles = append(sourceFiles, sf)
	}
	return NewTypeParser(c.Program(), c), sourceFiles, done
}

func TestReferenceSymbolAtNode_MatchesUngatedComputation(t *testing.T) {
	t.Parallel()

	sources := moduleExportReferenceSources()
	sources["/.src/syntax.ts"] = `import * as Module from "cache-fixture"
import { Foo as RenamedFoo, "Bar" as QuotedBar } from "cache-fixture"
export { RenamedFoo as ReexportedFoo }
const key = Symbol()
class Base { value = Module.Foo }
class Derived extends Base {
  #private = Module.Foo;
  "literal" = Module.Foo;
  0 = Module.Foo;
  1n = Module.Foo;
  ["computed"] = Module.Foo;
  [key] = Module.Foo;
  constructor() { super() }
  method(): this {
    super.value
    this.#private
    #private in this
    return this
  }
}
type Imported = typeof import("cache-fixture")
type ImportedMember = import("cache-fixture").Foo
type Qualified = typeof Module.Foo
type Indexed = { a: 1 }["a"]
const object = { "literal": Module.Foo, 0: Module.Foo, [` + "`template`" + `]: 1 }
object["literal"]
object[0]
object[` + "`literal`" + `]
delete (object as any).literal
const { literal: alias, ...rest } = object
const [first] = [alias]
const arrow = () => alias
const big = 1n
new Derived() instanceof Base
function target() { return new.target }
const metadata = import.meta
const loaded = import("cache-fixture")
label: for (const item of [first]) { if (item) break label; else continue label }
switch (big) { case 1n: break; default: }
try { throw QuotedBar } catch { } finally { }
enum Color { Red, "Green" }
namespace Space { export const inner = Color.Red }
declare module "ambient" { export const value: number }
export default class Default {}`
	sources["/.src/docs.ts"] = `import * as Module from "cache-fixture"
/**
 * @template T
 * @param {T} value
 * @returns {T}
 * @see Module.Foo
 * @see {@link Module.Bar}
 */
export function identity<T>(value: T): T { return value }
/** @typedef {{ value: typeof Module.Foo }} Options */
export const options = { value: Module.Foo }`
	sources["/.src/export-equals.ts"] = `const api = { run() {} }; export = api`
	sources["/.src/export-default.ts"] = `export default function () { return 1 }`
	sources["/.src/jsdoc.js"] = `// @ts-check
import * as Module from "cache-fixture"
/** @import { Foo } from "cache-fixture" */
/** @typedef {{ a: number, foo: typeof Module.Foo }} Shape */
/** @callback Mapper
 * @param {number} n
 * @returns {string}
 */
/**
 * @template {Shape} T
 * @param {T} value
 * @param {Mapper} [map]
 * @returns {T["a"]}
 */
export function pick(value, map) { return value.a }
/** @type {Shape} */
export const shape = { a: 1, foo: Module.Foo }
export const cast = /** @type {number} */ (/** @type {unknown} */ (shape.a))
export const sat = /** @satisfies {Shape} */ ({ a: 2, foo: Module.Foo })
export class Box {
  /** @type {number} */
  size = 1
  /** @param {number} n @this {Box} */
  grow(n) { this.size += n; return this }
}
/** @enum {string} */
export const Mode = { A: "a" }
module.exports.extra = 1
exports["other"] = 2`
	sources["/.src/view.tsx"] = `declare global { namespace JSX { interface IntrinsicElements { div: any; "svg:rect": { "xlink:href"?: string } } } }
const Comp = (props: { label: string }) => <div data-label={props.label}>{props.label}</div>
export const view = <><Comp label="x" /><svg:rect xlink:href="#a" /></>`

	tp, sourceFiles, done := compileFixturesWithJS(t, "", sources)
	defer done()

	kinds := make(map[ast.Kind]int)
	var symbols, uncached, jsdoc int
	for _, sf := range sourceFiles {
		if _, ok := sources[string(sf.FileName())]; !ok {
			continue
		}
		var visit ast.Visitor
		visit = func(node *ast.Node) bool {
			kinds[node.Kind]++
			want := tp.referenceSymbolFrom(node, tp.GetSymbolAtLocation(node))
			for i := range 2 {
				if got := tp.ReferenceSymbolAtNode(node); got != want {
					t.Errorf("%s:%d %v: lookup %d got symbol %v, want %v", sf.FileName(), node.Pos(), node.Kind, i, got, want)
				}
			}
			if want != nil {
				symbols++
			}
			if node.Flags&ast.NodeFlagsJSDoc != 0 {
				jsdoc++
				if !tp.links.ReferenceSymbol.Has(node) {
					t.Errorf("%s:%d %v: JSDoc node skipped the ungated path", sf.FileName(), node.Pos(), node.Kind)
				}
			} else if !canHaveSymbolAtLocation(node.Kind) {
				uncached++
				if tp.links.ReferenceSymbol.Has(node) {
					t.Errorf("%s:%d %v: gated node has a cache entry", sf.FileName(), node.Pos(), node.Kind)
				}
			}
			ast.ForEachChildAndJSDoc(node, sf, visit)
			return false
		}
		visit(sf.AsNode())
	}
	if symbols == 0 || uncached == 0 || jsdoc == 0 {
		t.Fatalf("expected symbols, gated nodes and JSDoc nodes, got %d, %d, %d", symbols, uncached, jsdoc)
	}
	for _, kind := range []ast.Kind{
		ast.KindSourceFile, ast.KindIdentifier, ast.KindPrivateIdentifier,
		ast.KindStringLiteral, ast.KindNumericLiteral, ast.KindBigIntLiteral,
		ast.KindNoSubstitutionTemplateLiteral, ast.KindComputedPropertyName,
		ast.KindPropertyAccessExpression, ast.KindElementAccessExpression, ast.KindQualifiedName,
		ast.KindThisKeyword, ast.KindThisType, ast.KindSuperKeyword,
		ast.KindDefaultKeyword, ast.KindExportKeyword, ast.KindEqualsGreaterThanToken,
		ast.KindImportType, ast.KindMetaProperty, ast.KindJsxNamespacedName,
		ast.KindExportAssignment, ast.KindBinaryExpression,
		ast.KindJSDoc, ast.KindJSDocParameterTag, ast.KindJSDocReturnTag, ast.KindJSDocTypeTag,
		ast.KindJSDocTemplateTag, ast.KindJSDocTypedefTag, ast.KindJSDocCallbackTag, ast.KindJSDocImportTag,
		ast.KindBlock, ast.KindCallExpression, ast.KindVariableStatement, ast.KindLabeledStatement,
		ast.KindCaseClause, ast.KindCatchClause, ast.KindHeritageClause, ast.KindJsxAttribute,
	} {
		if kinds[kind] == 0 {
			t.Errorf("fixture did not exercise %v", kind)
		}
	}
}

func TestSymbolHasDeclarationInModuleCached_DeclarationCounts(t *testing.T) {
	t.Parallel()

	sources := moduleExportReferenceSources()
	sources["/node_modules/cache-fixture/source.d.ts"] += `
export interface Merged { first: string }
export interface Merged { second: number }`
	sources["/node_modules/cache-fixture/index.d.ts"] += `
export type { Merged } from "./source.js"`
	sources["/.src/main.ts"] += `
import type { Merged } from "cache-fixture"
declare const merged: Merged`
	_, tp, sourceFiles, done := compileAndGetCheckerAndSourceFilesInternal(t, sources)
	defer done()

	for _, tc := range []struct {
		name         string
		occurrence   int
		declarations int
	}{
		{"RootFoo", 1, 1},
		{"Merged", 1, 2},
	} {
		sym := tp.ReferenceSymbolAtNode(findIdentifierByText(t, sourceFiles["/.src/main.ts"], tc.name, tc.occurrence))
		if sym == nil || len(ast.TSGetSymbolDeclarations(sym)) != tc.declarations {
			t.Fatalf("expected %d declarations for %s, got %v", tc.declarations, tc.name, sym)
		}
		for _, desc := range []packageSourceFileDescriptor{
			newPackageSourceFileDescriptor(testDescriptorSourceFile, "cache-fixture", sourceFileNameMatcher("/source.d.ts", nil)),
			newPackageSourceFileDescriptor(testDescriptorAnyFile, "cache-fixture", nil),
			newPackageSourceFileDescriptor(testDescriptorOtherFile, "cache-fixture", sourceFileNameMatcher("/other.d.ts", nil)),
		} {
			want := desc.id != testDescriptorOtherFile
			for range 2 {
				if got := tp.symbolHasDeclarationInModuleCached(sym, desc); got != want {
					t.Errorf("%s descriptor %d: got %v, want %v", tc.name, desc.id, got, want)
				}
			}
		}
		if cached := tp.links.ModuleExportFilter.TryGet(sym) != nil; cached != (tc.declarations > 1) {
			t.Errorf("%s: per-symbol filter cached = %v for %d declarations", tc.name, cached, tc.declarations)
		}
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
