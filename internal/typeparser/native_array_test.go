package typeparser

import (
	"strings"
	"testing"
	"testing/fstest"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/bundled"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

func findNativeArrayAccess(t testing.TB, sf *ast.SourceFile, text string) *ast.Node {
	t.Helper()
	var found *ast.Node
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if (node.Kind == ast.KindPropertyAccessExpression || node.Kind == ast.KindElementAccessExpression) &&
			strings.TrimSpace(sf.Text()[node.Pos():node.End()]) == text {
			found = node
			return false
		}
		node.ForEachChild(walk)
		return false
	}
	walk(sf.AsNode())
	if found == nil {
		t.Fatalf("member access %q not found", text)
	}
	return found
}

func TestIsArrayType(t *testing.T) {
	t.Parallel()
	c, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `
type Numbers = number[];
declare const mutable: number[];
declare const readonly: readonly number[];
declare const alias: Numbers;
declare const tuple: [number, string];
declare const readonlyTuple: readonly [number, string];
declare const union: number[] | string[];
declare const optional: number[] | undefined;
declare const typed: Uint8Array;
declare const anyValue: any;
class Inherited extends Array<number> {}
declare const inherited: Inherited;
function constrained<T extends readonly number[]>(generic: T) {}
`)
	defer done()
	for _, tc := range []struct {
		name string
		want bool
	}{
		{"mutable", true}, {"readonly", true}, {"alias", true},
		{"tuple", false}, {"readonlyTuple", false}, {"union", false},
		{"optional", false}, {"typed", false}, {"anyValue", false},
		{"inherited", false}, {"generic", false},
	} {
		typ := c.GetTypeAtLocation(findIdentifierByText(t, sf, tc.name, 0))
		if got := tp.IsArrayType(typ); got != tc.want {
			t.Errorf("IsArrayType(%s) = %v, want %v", tc.name, got, tc.want)
		}
	}
	if tp.IsArrayType(nil) {
		t.Error("IsArrayType(nil) = true")
	}
}

func TestNativeArrayMethodReference(t *testing.T) {
	t.Parallel()
	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `
declare const values: number[];
declare const readonly: readonly number[];
declare const tuple: readonly [number, string];
declare const optional: readonly number[] | undefined;
declare const nativeUnion: number[] | readonly string[];
declare const mixed: number[] | { map: (f: (n: number) => number) => number[] };
class Inherited extends Array<number> {}
declare const inherited: Inherited;
class Override extends Array<number> { map<U>(f: (n: number, i: number, a: number[]) => U): U[] { return []; } }
declare const override: Override;
declare const typed: Uint8Array;
declare const fn: () => void;
declare const object: object;
declare const key: "map";
type Remapped = { [K in keyof number[] as K extends "filter" ? "map" : never]: number[][K] };
declare const remapped: Remapped;
declare const conflicting: Remapped | number[];
function constrained<T extends readonly number[]>(generic: T) { generic["map"]; }
function shadowed(Array: { isArray: (value: unknown) => boolean }) { Array.isArray; }
const Native = Array;
values.map;
values.push;
readonly.filter;
tuple.map;
tuple["map"];
optional?.filter;
optional?.["filter"];
values[`+"`map`"+`];
nativeUnion.map;
mixed.map;
inherited.map;
override.map;
typed.map;
fn.call;
object.hasOwnProperty;
Native.isArray;
Native["isArray"];
Native.from;
Native.call;
values[key];
remapped.map;
conflicting.map;
const alias = values.map;
alias;
`)
	defer done()
	for _, tc := range []struct {
		access      string
		name        string
		constructor bool
		want        bool
	}{
		{"Array.isArray", "isArray", true, false},
		{"values.map", "map", false, true},
		{"values.push", "push", false, true},
		{"readonly.filter", "filter", false, true},
		{"tuple.map", "map", false, true},
		{"tuple[\"map\"]", "map", false, true},
		{"optional?.filter", "filter", false, true},
		{"optional?.[\"filter\"]", "filter", false, true},
		{"values[`map`]", "map", false, true},
		{"generic[\"map\"]", "map", false, true},
		{"nativeUnion.map", "map", false, true},
		{"mixed.map", "map", false, false},
		{"inherited.map", "map", false, true},
		{"override.map", "map", false, false},
		{"typed.map", "map", false, false},
		{"fn.call", "call", true, false},
		{"fn.call", "call", false, false},
		{"object.hasOwnProperty", "hasOwnProperty", false, false},
		{"Native.isArray", "isArray", true, true},
		{"Native[\"isArray\"]", "isArray", true, true},
		{"Native.from", "from", true, true},
		{"Native.call", "call", true, false},
		{"values[key]", "map", false, false},
		{"remapped.map", "filter", false, true},
		{"remapped.map", "map", false, false},
		{"conflicting.map", "map", false, false},
		{"conflicting.map", "filter", false, false},
	} {
		node := findNativeArrayAccess(t, sf, tc.access)
		query, otherNamespace := tp.IsNativeArrayMethodReference, tp.IsNativeArrayConstructorMethodReference
		if tc.constructor {
			query, otherNamespace = otherNamespace, query
		}
		if query(node, "unknownMethod") {
			t.Errorf("%s matched an unknown method before classification", tc.access)
		}
		if got := query(node, tc.name); got != tc.want {
			t.Errorf("%s as %s = %v, want %v", tc.access, tc.name, got, tc.want)
		}
		if query(node, "unknownMethod") || otherNamespace(node, tc.name) {
			t.Errorf("%s classification leaked into another name or namespace", tc.access)
		}
		if got := query(node, tc.name); got != tc.want {
			t.Errorf("%s cached as %s = %v, want %v", tc.access, tc.name, got, tc.want)
		}
	}
	if tp.IsNativeArrayMethodReference(nil, "map") ||
		tp.IsNativeArrayMethodReference(findIdentifierByText(t, sf, "alias", 1), "map") ||
		tp.IsNativeArrayMethodReference(findNativeArrayAccess(t, sf, "values.map"), "") {
		t.Error("unsupported reference matched a native Array method")
	}
}

func TestNativeArrayAugmentation(t *testing.T) {
	t.Parallel()
	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `
export {};
declare global {
  interface Array<T> { map<U>(f: (value: T) => U): U[]; }
  interface ReadonlyArray<T> { map<U>(f: (value: T) => U): U[]; }
  interface ArrayConstructor { isArray(value: number): boolean; }
}
declare const values: number[];
declare const readonly: readonly number[];
values.map;
readonly.map;
values.filter;
readonly.filter;
Array.isArray;
Array.of;
`)
	defer done()
	for _, access := range []string{"values.map", "readonly.map"} {
		if tp.IsNativeArrayMethodReference(findNativeArrayAccess(t, sf, access), "map") {
			t.Errorf("augmented member %s matched", access)
		}
	}
	for _, access := range []string{"values.filter", "readonly.filter"} {
		if !tp.IsNativeArrayMethodReference(findNativeArrayAccess(t, sf, access), "filter") {
			t.Errorf("unaugmented member %s did not match", access)
		}
	}
	if tp.IsNativeArrayConstructorMethodReference(findNativeArrayAccess(t, sf, "Array.isArray"), "isArray") ||
		!tp.IsNativeArrayConstructorMethodReference(findNativeArrayAccess(t, sf, "Array.of"), "of") {
		t.Error("constructor augmentation affected the wrong members")
	}
}

func TestNativeArrayInheritedAugmentation(t *testing.T) {
	t.Parallel()
	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `
export {};
declare global {
  interface Array<T> extends Pick<ArrayConstructor, "isArray"> {}
}
Array.isArray;
`)
	defer done()
	node := findNativeArrayAccess(t, sf, "Array.isArray")
	if !tp.IsNativeArrayConstructorMethodReference(node, "isArray") {
		t.Error("Array.isArray constructor reference = false, want true")
	}
	if tp.IsNativeArrayMethodReference(node, "isArray") {
		t.Error("Array.isArray instance reference = true, want false")
	}
}

func TestNativeArrayNoLib(t *testing.T) {
	t.Parallel()
	const source = `
interface Array<T> { map(f: (value: T) => T): T[]; }
interface ArrayConstructor { isArray(value: unknown): boolean; }
declare const Array: ArrayConstructor;
declare const values: number[];
values.map;
Array.isArray;
`
	fs := bundled.WrapFS(vfstest.FromMap(map[string]any{
		"/.src/test.ts": &fstest.MapFile{Data: []byte(source)},
	}, tspath.CaseSensitive))
	program := compiler.NewProgram(compiler.ProgramOptions{
		Config: tsoptions.NewParsedCommandLine(&core.CompilerOptions{
			NoLib: core.TSTrue, Target: core.ScriptTargetESNext, Strict: core.TSTrue,
		}, []tspath.RootedFilePath{"/.src/test.ts"}, nil, "/", tspath.CaseSensitive),
		Host:           compiler.NewCompilerHost(fs, bundled.LibPath(), nil, nil, nil),
		SingleThreaded: core.TSTrue,
	})
	c, done := program.GetTypeChecker(t.Context())
	defer done()
	tp := NewTypeParser(program, c)
	sf := program.GetSourceFile("/.src/test.ts")
	if tp.IsNativeArrayMethodReference(findNativeArrayAccess(t, sf, "values.map"), "map") ||
		tp.IsNativeArrayConstructorMethodReference(findNativeArrayAccess(t, sf, "Array.isArray"), "isArray") {
		t.Error("noLib user declarations matched native Array members")
	}
	if len(tp.links.nativeArrayRegistry.instance) != 0 || len(tp.links.nativeArrayRegistry.constructor) != 0 {
		t.Error("noLib registry contains canonical members")
	}
}

func TestNativeArrayCheckerCache(t *testing.T) {
	t.Parallel()
	const source = `export {};
declare const values: number[];
declare const custom: { map: () => void };
values.map;
custom.map;
`
	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/first.ts": source, "/.src/second.ts": source,
	})
	defer done()
	if tp.links.nativeArrayRegistry != nil {
		t.Fatal("registry was built before a reference query")
	}
	if !tp.IsArrayType(c.GetTypeAtLocation(findIdentifierByText(t, files["/.src/first.ts"], "values", 0))) || tp.links.nativeArrayRegistry != nil {
		t.Fatal("IsArrayType changed reference registry initialization")
	}
	second := NewTypeParser(c.Program(), c)
	for _, access := range []string{"values.map", "custom.map"} {
		node := findNativeArrayAccess(t, files["/.src/first.ts"], access)
		want := access == "values.map"
		if got := tp.IsNativeArrayMethodReference(node, "map"); got != want {
			t.Fatalf("first parser %s = %v, want %v", access, got, want)
		}
		symbol := c.GetSymbolAtLocation(node)
		cached := tp.links.nativeArrayMember.TryGet(symbol)
		wantMember := nativeArrayMember{}
		if want {
			wantMember = nativeArrayMember{nativeArrayInstance, "map"}
		}
		if cached == nil || *cached != wantMember {
			t.Fatalf("%s classification was not cached", access)
		}
		if got := second.IsNativeArrayMethodReference(node, "map"); got != want || second.links != tp.links || second.links.nativeArrayMember.TryGet(symbol) != cached {
			t.Fatalf("second parser did not reuse the %s classification", access)
		}
	}
	registry := tp.links.nativeArrayRegistry
	if !second.IsNativeArrayMethodReference(findNativeArrayAccess(t, files["/.src/second.ts"], "values.map"), "map") || second.links.nativeArrayRegistry != registry {
		t.Fatal("second source file did not reuse the canonical registry")
	}
	otherChecker, other, otherFile, otherDone := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer otherDone()
	firstSymbol := c.GetSymbolAtLocation(findNativeArrayAccess(t, files["/.src/first.ts"], "values.map"))
	if other.links == tp.links || other.links.nativeArrayRegistry != nil || other.links.nativeArrayMember.TryGet(firstSymbol) != nil {
		t.Fatal("separate checker inherited native Array cache entries")
	}
	otherNode := findNativeArrayAccess(t, otherFile, "values.map")
	if !other.IsNativeArrayMethodReference(otherNode, "map") || other.links.nativeArrayRegistry == registry || otherChecker.GetSymbolAtLocation(otherNode) == firstSymbol {
		t.Fatal("separate checker did not build its own native Array cache")
	}
}
