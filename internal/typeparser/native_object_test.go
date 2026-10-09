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

func findNativeObjectAccess(t testing.TB, sf *ast.SourceFile, text string) *ast.Node {
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

func TestNativeObjectMethodReference(t *testing.T) {
	t.Parallel()
	_, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/aliases.ts": `export const Native = Object;`,
		"/.src/test.ts": `
import { Native as Imported } from "./aliases";
declare const object: object;
declare const optional: object | undefined;
declare const optionalConstructor: ObjectConstructor | undefined;
declare const picked: Pick<ObjectConstructor, "keys">;
declare const mixed: ObjectConstructor | { keys: typeof Object.keys };
declare const mixedInstance: object | { hasOwnProperty: (key: PropertyKey) => boolean };
declare const intersection: ObjectConstructor & { keys: (value: number) => string[] };
declare const custom: { keys: typeof Object.keys };
declare const dynamic: "keys";
declare const anything: any;
declare const arrays: number[];
declare const map: Map<string, number>;
class Inherited {}
class Override { hasOwnProperty(key: PropertyKey): boolean { return false; } }
declare const inherited: Inherited;
declare const override: Override;
type Remapped = { [K in keyof ObjectConstructor as K extends "values" ? "keys" : never]: ObjectConstructor[K] };
declare const remapped: Remapped;
declare const conflicting: Remapped | Pick<ObjectConstructor, "keys">;
function constrained<T extends ObjectConstructor>(generic: T) { generic["entries"]; }
function constrainedInstance<T extends object>(genericInstance: T) { genericInstance["hasOwnProperty"]; }
function customConstraint<T extends { keys: (value: object) => string[] }>(customGeneric: T) { customGeneric["keys"]; }
function mixedConstraint<T extends ObjectConstructor | { keys: (value: object) => string[] }>(mixedGeneric: T) { mixedGeneric["keys"]; }
function shadowed(Object: { keys: typeof globalThis.Object.keys }) { Object?.keys; }
namespace Local {
  interface ObjectConstructor { keys(value: object): string[]; }
  declare const local: ObjectConstructor;
  local.keys;
}
const Native = Object;
Object.keys;
Object.values;
Object.entries;
Object.fromEntries;
Object.hasOwn;
Object["keys"];
Object[` + "`values`" + `];
Object.prototype.hasOwnProperty.call({}, "a");
Object.hasOwnProperty;
object.hasOwnProperty;
object["hasOwnProperty"];
optional?.hasOwnProperty;
optional?.["hasOwnProperty"];
optionalConstructor?.keys;
optionalConstructor?.["keys"];
inherited.hasOwnProperty;
override.hasOwnProperty;
Native.keys;
Native["entries"];
Imported.keys;
globalThis.Object.hasOwn;
globalThis.Object.keys;
picked.keys;
custom.keys;
mixed.keys;
mixedInstance.hasOwnProperty;
intersection.keys;
remapped.keys;
conflicting.keys;
Object[dynamic];
anything.keys;
arrays.keys;
arrays.entries;
map.keys;
const alias = Object.keys;
alias;
`,
	})
	defer done()
	sf := files["/.src/test.ts"]
	for _, tc := range []struct {
		access      string
		name        string
		constructor bool
		want        bool
	}{
		{"Object.keys", "keys", true, true},
		{"Object?.keys", "keys", true, false},
		{"globalThis.Object.keys", "keys", true, true},
		{"Object.values", "values", true, true},
		{"Object.entries", "entries", true, true},
		{"Object.fromEntries", "fromEntries", true, true},
		{"Object.hasOwn", "hasOwn", true, true},
		{"Object[\"keys\"]", "keys", true, true},
		{"Object[`values`]", "values", true, true},
		{"Object.prototype.hasOwnProperty", "hasOwnProperty", false, true},
		{"Object.hasOwnProperty", "hasOwnProperty", false, true},
		{"object.hasOwnProperty", "hasOwnProperty", false, true},
		{"object[\"hasOwnProperty\"]", "hasOwnProperty", false, true},
		{"optional?.hasOwnProperty", "hasOwnProperty", false, true},
		{"optional?.[\"hasOwnProperty\"]", "hasOwnProperty", false, true},
		{"optionalConstructor?.keys", "keys", true, true},
		{"optionalConstructor?.[\"keys\"]", "keys", true, true},
		{"inherited.hasOwnProperty", "hasOwnProperty", false, true},
		{"override.hasOwnProperty", "hasOwnProperty", false, false},
		{"Native.keys", "keys", true, true},
		{"Native[\"entries\"]", "entries", true, true},
		{"Imported.keys", "keys", true, true},
		{"globalThis.Object.hasOwn", "hasOwn", true, true},
		{"picked.keys", "keys", true, true},
		{"custom.keys", "keys", true, false},
		{"mixed.keys", "keys", true, false},
		{"mixedInstance.hasOwnProperty", "hasOwnProperty", false, false},
		{"intersection.keys", "keys", true, false},
		{"remapped.keys", "values", true, true},
		{"remapped.keys", "keys", true, false},
		{"conflicting.keys", "keys", true, false},
		{"conflicting.keys", "values", true, false},
		{"generic[\"entries\"]", "entries", true, true},
		{"genericInstance[\"hasOwnProperty\"]", "hasOwnProperty", false, true},
		{"customGeneric[\"keys\"]", "keys", true, false},
		{"mixedGeneric[\"keys\"]", "keys", true, false},
		{"local.keys", "keys", true, false},
		{"Object[dynamic]", "keys", true, false},
		{"anything.keys", "keys", true, false},
		{"arrays.keys", "keys", true, false},
		{"arrays.entries", "entries", true, false},
		{"map.keys", "keys", true, false},
	} {
		node := findNativeObjectAccess(t, sf, tc.access)
		query, otherNamespace := tp.IsNativeObjectMethodReference, tp.IsNativeObjectConstructorMethodReference
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
	if tp.IsNativeObjectMethodReference(nil, "hasOwnProperty") ||
		tp.IsNativeObjectConstructorMethodReference(findIdentifierByText(t, sf, "alias", 1), "keys") ||
		tp.IsNativeObjectConstructorMethodReference(findNativeObjectAccess(t, sf, "Native.keys"), "") {
		t.Error("unsupported reference matched a native Object method")
	}
	if !tp.IsNativeArrayMethodReference(findNativeObjectAccess(t, sf, "arrays.keys"), "keys") ||
		tp.IsNativeArrayMethodReference(findNativeObjectAccess(t, sf, "Native.keys"), "keys") ||
		tp.IsNativeArrayConstructorMethodReference(findNativeObjectAccess(t, sf, "Native.keys"), "keys") {
		t.Error("Object recognition changed Array classification")
	}
}

func TestNativeObjectAugmentation(t *testing.T) {
	t.Parallel()
	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `
export {};
declare global {
  interface ObjectConstructor { keys<T>(value: T): (keyof T)[]; }
  interface Object { hasOwnProperty(key: string): boolean; }
}
Object.keys;
Object.values;
Object.hasOwn;
Object.prototype.hasOwnProperty;
Object.prototype.isPrototypeOf;
`)
	defer done()
	if tp.IsNativeObjectConstructorMethodReference(findNativeObjectAccess(t, sf, "Object.keys"), "keys") ||
		!tp.IsNativeObjectConstructorMethodReference(findNativeObjectAccess(t, sf, "Object.values"), "values") ||
		!tp.IsNativeObjectConstructorMethodReference(findNativeObjectAccess(t, sf, "Object.hasOwn"), "hasOwn") {
		t.Error("constructor augmentation affected the wrong members")
	}
	if tp.IsNativeObjectMethodReference(findNativeObjectAccess(t, sf, "Object.prototype.hasOwnProperty"), "hasOwnProperty") ||
		!tp.IsNativeObjectMethodReference(findNativeObjectAccess(t, sf, "Object.prototype.isPrototypeOf"), "isPrototypeOf") {
		t.Error("instance augmentation affected the wrong members")
	}
}

func TestNativeObjectInheritedAugmentation(t *testing.T) {
	t.Parallel()
	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `
export {};
declare global { interface Object extends Pick<ObjectConstructor, "keys"> {} }
Object.keys;
`)
	defer done()
	node := findNativeObjectAccess(t, sf, "Object.keys")
	if !tp.IsNativeObjectConstructorMethodReference(node, "keys") || tp.IsNativeObjectMethodReference(node, "keys") {
		t.Error("inherited constructor roots changed Object.keys namespace")
	}
}

func TestNativeObjectNoLib(t *testing.T) {
	t.Parallel()
	const source = `
interface Object { hasOwnProperty(key: string): boolean; }
interface ObjectConstructor { keys(value: object): string[]; prototype: Object; }
declare const Object: ObjectConstructor;
Object.keys;
Object.prototype.hasOwnProperty;
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
	if tp.IsNativeObjectConstructorMethodReference(findNativeObjectAccess(t, sf, "Object.keys"), "keys") ||
		tp.IsNativeObjectMethodReference(findNativeObjectAccess(t, sf, "Object.prototype.hasOwnProperty"), "hasOwnProperty") {
		t.Error("noLib user declarations matched native Object members")
	}
	if len(tp.links.nativeObjectRegistry.instance) != 0 || len(tp.links.nativeObjectRegistry.constructor) != 0 {
		t.Error("noLib registry contains canonical members")
	}
}

func TestNativeObjectCheckerCache(t *testing.T) {
	t.Parallel()
	const source = `export {};
declare const custom: { keys: () => void };
Object.keys;
custom.keys;
`
	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/first.ts": source, "/.src/second.ts": source,
	})
	defer done()
	if tp.links.nativeObjectRegistry != nil {
		t.Fatal("registry was built before a reference query")
	}
	second := NewTypeParser(c.Program(), c)
	for _, access := range []string{"Object.keys", "custom.keys"} {
		node := findNativeObjectAccess(t, files["/.src/first.ts"], access)
		want := access == "Object.keys"
		if got := tp.IsNativeObjectConstructorMethodReference(node, "keys"); got != want {
			t.Fatalf("first parser %s = %v, want %v", access, got, want)
		}
		symbol := c.GetSymbolAtLocation(node)
		cached := tp.links.nativeObjectMember.TryGet(symbol)
		wantMember := nativeObjectMember{}
		if want {
			wantMember = nativeObjectMember{nativeObjectConstructor, "keys"}
		}
		if cached == nil || *cached != wantMember {
			t.Fatalf("%s classification was not cached", access)
		}
		if got := second.IsNativeObjectConstructorMethodReference(node, "keys"); got != want || second.links != tp.links || second.links.nativeObjectMember.TryGet(symbol) != cached {
			t.Fatalf("second parser did not reuse the %s classification", access)
		}
	}
	registry := tp.links.nativeObjectRegistry
	if !second.IsNativeObjectConstructorMethodReference(findNativeObjectAccess(t, files["/.src/second.ts"], "Object.keys"), "keys") || second.links.nativeObjectRegistry != registry {
		t.Fatal("second source file did not reuse the canonical registry")
	}
	otherChecker, other, otherFile, otherDone := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer otherDone()
	firstSymbol := c.GetSymbolAtLocation(findNativeObjectAccess(t, files["/.src/first.ts"], "Object.keys"))
	if other.links == tp.links || other.links.nativeObjectRegistry != nil || other.links.nativeObjectMember.TryGet(firstSymbol) != nil {
		t.Fatal("separate checker inherited native Object cache entries")
	}
	otherNode := findNativeObjectAccess(t, otherFile, "Object.keys")
	if !other.IsNativeObjectConstructorMethodReference(otherNode, "keys") || other.links.nativeObjectRegistry == registry || otherChecker.GetSymbolAtLocation(otherNode) == firstSymbol {
		t.Fatal("separate checker did not build its own native Object cache")
	}
}
