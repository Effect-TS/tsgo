package completions

import (
	"context"
	"testing"

	"github.com/effect-ts/tsgo/internal/completion"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/parser"
)

func parseSourceFile(source string) *ast.SourceFile {
	return parser.ParseSourceFile(ast.SourceFileParseOptions{
		FileName: "/test.ts",
	}, source, core.ScriptKindTS)
}

func makeFnContext(source string, position int) *completion.Context {
	sf := parseSourceFile(source)
	return completion.NewContext(context.Background(), sf, position, nil, nil, nil)
}

func TestFnFunctionStar_NotEffectModule(t *testing.T) {
	t.Parallel()
	// Accessing a non-Effect identifier should return nil
	source := `import * as Foo from "some-lib"
const x = Foo.fn`
	ctx := makeFnContext(source, len(source))

	items := runFnFunctionStar(ctx)
	if items != nil {
		t.Errorf("expected nil for non-Effect module access, got %d items", len(items))
	}
}

func TestFnFunctionStar_NamespaceImport_InVarDecl(t *testing.T) {
	t.Parallel()
	// Effect.fn| inside a variable declaration should produce 3 items (named + generic + untraced)
	source := `import * as Effect from "effect/Effect"
const myFn = Effect.fn`
	ctx := makeFnContext(source, len(source))

	items := runFnFunctionStar(ctx)
	if len(items) != 3 {
		t.Fatalf("expected 3 items, got %d", len(items))
	}
	// First item should be the named variant with "myFn"
	if items[0].Label != `fn("myFn")` {
		t.Errorf("item[0].Label = %q, want %q", items[0].Label, `fn("myFn")`)
	}
	if items[1].Label != "fn(function*(){})" {
		t.Errorf("item[1].Label = %q, want %q", items[1].Label, "fn(function*(){})")
	}
	if items[2].Label != "fnUntraced(function*(){})" {
		t.Errorf("item[2].Label = %q, want %q", items[2].Label, "fnUntraced(function*(){})")
	}
}

func TestFnFunctionStar_NamedImport_InVarDecl(t *testing.T) {
	t.Parallel()
	// import { Effect } from "effect" — should also work
	source := `import { Effect } from "effect"
const myFn = Effect.fn`
	ctx := makeFnContext(source, len(source))

	items := runFnFunctionStar(ctx)
	if len(items) != 3 {
		t.Fatalf("expected 3 items, got %d", len(items))
	}
	if items[0].Label != `fn("myFn")` {
		t.Errorf("item[0].Label = %q, want %q", items[0].Label, `fn("myFn")`)
	}
}

func TestFnFunctionStar_NotInVarDecl(t *testing.T) {
	t.Parallel()
	// Effect.fn| not inside a variable declaration — should produce 2 items (no named variant)
	source := `import * as Effect from "effect/Effect"
Effect.fn`
	ctx := makeFnContext(source, len(source))

	items := runFnFunctionStar(ctx)
	if len(items) != 2 {
		t.Fatalf("expected 2 items (no named variant), got %d", len(items))
	}
	if items[0].Label != "fn(function*(){})" {
		t.Errorf("item[0].Label = %q, want %q", items[0].Label, "fn(function*(){})")
	}
	if items[1].Label != "fnUntraced(function*(){})" {
		t.Errorf("item[1].Label = %q, want %q", items[1].Label, "fnUntraced(function*(){})")
	}
}

func TestFnFunctionStar_AliasedImport(t *testing.T) {
	t.Parallel()
	// Aliased namespace import: import * as Fx from "effect/Effect" — Fx.fn| should work
	source := `import * as Fx from "effect/Effect"
const myFn = Fx.fn`
	ctx := makeFnContext(source, len(source))

	items := runFnFunctionStar(ctx)
	if len(items) != 3 {
		t.Fatalf("expected 3 items for aliased import, got %d", len(items))
	}
	if items[0].Label != `fn("myFn")` {
		t.Errorf("item[0].Label = %q, want %q", items[0].Label, `fn("myFn")`)
	}
}

func TestFnFunctionStar_CursorAfterDot(t *testing.T) {
	t.Parallel()
	// Cursor right after the dot (no partial text typed)
	source := `import * as Effect from "effect/Effect"
const myFn = Effect.`
	ctx := makeFnContext(source, len(source))

	items := runFnFunctionStar(ctx)
	if len(items) != 3 {
		t.Fatalf("expected 3 items for cursor after dot, got %d", len(items))
	}
}

func TestFnFunctionStar_SnippetInsertText(t *testing.T) {
	t.Parallel()
	// Verify the insert text contains proper snippet placeholders
	source := `import * as Effect from "effect/Effect"
const myFn = Effect.fn`
	ctx := makeFnContext(source, len(source))

	items := runFnFunctionStar(ctx)
	if len(items) != 3 {
		t.Fatalf("expected 3 items, got %d", len(items))
	}

	// Check insert text for named variant
	namedInsert := items[0].TextEdit.TextEdit.NewText
	expected := `fn("myFn")(function*(${1}){${0}})`
	if namedInsert != expected {
		t.Errorf("named insert text = %q, want %q", namedInsert, expected)
	}

	// Check insert text for generic variant
	genericInsert := items[1].TextEdit.TextEdit.NewText
	expected = "fn(function*(${1}){${0}})"
	if genericInsert != expected {
		t.Errorf("generic insert text = %q, want %q", genericInsert, expected)
	}

	// Check insert text for untraced variant
	untracedInsert := items[2].TextEdit.TextEdit.NewText
	expected = "fnUntraced(function*(${1}){${0}})"
	if untracedInsert != expected {
		t.Errorf("untraced insert text = %q, want %q", untracedInsert, expected)
	}
}
