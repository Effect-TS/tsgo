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

func findNativeStringAccess(t testing.TB, sf *ast.SourceFile, text string) *ast.Node {
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

func TestNativeStringMethodReference(t *testing.T) {
	t.Parallel()
	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `
declare const value: string;
declare const literal: "hello";
declare const template: `+"`hello ${string}`"+`;
declare const branded: string & { _brand: "X" };
declare const boxed: String;
declare const optional: string | undefined;
declare const mixed: string | Array<string>;
declare const anyValue: any;
declare const key: "trim";
function constrained<T extends string>(generic: T) { generic.trim; }
value.trim;
literal.trim;
template.trim;
branded.trim;
boxed.trim;
optional?.trim;
optional?.["trim"];
value["trim"];
value[`+"`trim`"+`];
mixed.includes;
mixed.slice;
anyValue.trim;
value[key];
String.fromCharCode;
const alias = value.trim;
alias;
`)
	defer done()
	for _, tc := range []struct {
		access string
		name   string
		want   bool
	}{
		{"value.trim", "trim", true},
		{"literal.trim", "trim", true},
		{"template.trim", "trim", true},
		{"branded.trim", "trim", true},
		{"boxed.trim", "trim", true},
		{"generic.trim", "trim", true},
		{"optional?.trim", "trim", true},
		{"optional?.[\"trim\"]", "trim", true},
		{"value[\"trim\"]", "trim", true},
		{"value[`trim`]", "trim", true},
		{"mixed.includes", "includes", false},
		{"mixed.slice", "slice", false},
		{"anyValue.trim", "trim", false},
		{"value[key]", "trim", false},
		{"String.fromCharCode", "fromCharCode", false},
	} {
		node := findNativeStringAccess(t, sf, tc.access)
		if tp.IsNativeStringMethodReference(node, "unknownMethod") {
			t.Errorf("%s matched an unknown method before classification", tc.access)
		}
		if got := tp.IsNativeStringMethodReference(node, tc.name); got != tc.want {
			t.Errorf("%s as %s = %v, want %v", tc.access, tc.name, got, tc.want)
		}
		if tp.IsNativeStringMethodReference(node, "unknownMethod") {
			t.Errorf("%s classification leaked into another name", tc.access)
		}
		if got := tp.IsNativeStringMethodReference(node, tc.name); got != tc.want {
			t.Errorf("%s cached as %s = %v, want %v", tc.access, tc.name, got, tc.want)
		}
	}
	if tp.IsNativeStringMethodReference(nil, "trim") ||
		tp.IsNativeStringMethodReference(findIdentifierByText(t, sf, "alias", 1), "trim") ||
		tp.IsNativeStringMethodReference(findNativeStringAccess(t, sf, "value.trim"), "") {
		t.Error("unsupported reference matched a native String method")
	}
}

func TestNativeStringAugmentation(t *testing.T) {
	t.Parallel()
	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `
export {};
declare global { interface String { trim(): string; } }
declare const value: string;
value.trim;
value["trim"];
value.toUpperCase;
`)
	defer done()
	for _, access := range []string{"value.trim", "value[\"trim\"]"} {
		if tp.IsNativeStringMethodReference(findNativeStringAccess(t, sf, access), "trim") {
			t.Errorf("augmented member %s matched", access)
		}
	}
	if !tp.IsNativeStringMethodReference(findNativeStringAccess(t, sf, "value.toUpperCase"), "toUpperCase") {
		t.Error("unaugmented member did not match")
	}
}

func TestNativeStringNoLib(t *testing.T) {
	t.Parallel()
	const source = `
interface String { trim(): string; }
declare const value: string;
value.trim;
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
	if tp.IsNativeStringMethodReference(findNativeStringAccess(t, sf, "value.trim"), "trim") {
		t.Error("noLib user declarations matched native String members")
	}
	if len(tp.links.nativeStringRegistry.instance) != 0 {
		t.Error("noLib registry contains canonical members")
	}
}

func TestNativeStringCheckerCache(t *testing.T) {
	t.Parallel()
	const source = `export {};
declare const values: string;
declare const custom: { trim: () => void };
values.trim;
custom.trim;
`
	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/first.ts": source, "/.src/second.ts": source,
	})
	defer done()
	if tp.links.nativeStringRegistry != nil {
		t.Fatal("registry was built before a reference query")
	}
	second := NewTypeParser(c.Program(), c)
	for _, access := range []string{"values.trim", "custom.trim"} {
		node := findNativeStringAccess(t, files["/.src/first.ts"], access)
		want := access == "values.trim"
		if got := tp.IsNativeStringMethodReference(node, "trim"); got != want {
			t.Fatalf("first parser %s = %v, want %v", access, got, want)
		}
		symbol := tp.GetSymbolAtLocation(node)
		cached := tp.links.nativeStringMember.TryGet(symbol)
		wantMember := nativeStringMember{}
		if want {
			wantMember = nativeStringMember{"trim"}
		}
		if cached == nil || *cached != wantMember {
			t.Fatalf("%s classification was not cached", access)
		}
		if got := second.IsNativeStringMethodReference(node, "trim"); got != want || second.links != tp.links || second.links.nativeStringMember.TryGet(symbol) != cached {
			t.Fatalf("second parser did not reuse the %s classification", access)
		}
	}
	registry := tp.links.nativeStringRegistry
	if !second.IsNativeStringMethodReference(findNativeStringAccess(t, files["/.src/second.ts"], "values.trim"), "trim") || second.links.nativeStringRegistry != registry {
		t.Fatal("second source file did not reuse the canonical registry")
	}
	_, other, otherFile, otherDone := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer otherDone()
	firstSymbol := tp.GetSymbolAtLocation(findNativeStringAccess(t, files["/.src/first.ts"], "values.trim"))
	if other.links == tp.links || other.links.nativeStringRegistry != nil || other.links.nativeStringMember.TryGet(firstSymbol) != nil {
		t.Fatal("separate checker inherited native String cache entries")
	}
	otherNode := findNativeStringAccess(t, otherFile, "values.trim")
	if !other.IsNativeStringMethodReference(otherNode, "trim") || other.links.nativeStringRegistry == registry || other.GetSymbolAtLocation(otherNode) == firstSymbol {
		t.Fatal("separate checker did not build its own native String cache")
	}
}
