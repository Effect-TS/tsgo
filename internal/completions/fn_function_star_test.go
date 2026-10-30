package completions_test

import (
	"testing"
)

func TestFnFunctionStar_NotEffectModule(t *testing.T) {
	t.Parallel()
	source := `import * as Foo from "some-lib"
const x = Foo.fn`
	items := fnFunctionStarItems(t, source, len(source))
	if items != nil {
		t.Errorf("expected nil for non-Effect module access, got %d items", len(items))
	}
}

func TestFnFunctionStar_NamespaceImport_InVarDecl(t *testing.T) {
	t.Parallel()
	source := `import * as Effect from "effect/Effect"
const myFn = Effect.fn`
	items := fnFunctionStarItems(t, source, len(source))
	if len(items) != 3 {
		t.Fatalf("expected 3 items, got %d", len(items))
	}
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
	source := `import { Effect } from "effect"
const myFn = Effect.fn`
	items := fnFunctionStarItems(t, source, len(source))
	if len(items) != 3 {
		t.Fatalf("expected 3 items, got %d", len(items))
	}
	if items[0].Label != `fn("myFn")` {
		t.Errorf("item[0].Label = %q, want %q", items[0].Label, `fn("myFn")`)
	}
}

func TestFnFunctionStar_NotInVarDecl(t *testing.T) {
	t.Parallel()
	source := `import * as Effect from "effect/Effect"
Effect.fn`
	items := fnFunctionStarItems(t, source, len(source))
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
	source := `import * as Fx from "effect/Effect"
const myFn = Fx.fn`
	items := fnFunctionStarItems(t, source, len(source))
	if len(items) != 3 {
		t.Fatalf("expected 3 items for aliased import, got %d", len(items))
	}
	if items[0].Label != `fn("myFn")` {
		t.Errorf("item[0].Label = %q, want %q", items[0].Label, `fn("myFn")`)
	}
}

func TestFnFunctionStar_CursorAfterDot(t *testing.T) {
	t.Parallel()
	source := `import * as Effect from "effect/Effect"
const myFn = Effect.`
	items := fnFunctionStarItems(t, source, len(source))
	if len(items) != 3 {
		t.Fatalf("expected 3 items for cursor after dot, got %d", len(items))
	}
}

func TestFnFunctionStar_SnippetInsertText(t *testing.T) {
	t.Parallel()
	source := `import * as Effect from "effect/Effect"
const myFn = Effect.fn`
	items := fnFunctionStarItems(t, source, len(source))
	if len(items) != 3 {
		t.Fatalf("expected 3 items, got %d", len(items))
	}

	namedInsert := items[0].TextEdit.TextEdit.NewText
	expected := `fn("myFn")(function*(${1}){${0}})`
	if namedInsert != expected {
		t.Errorf("named insert text = %q, want %q", namedInsert, expected)
	}

	genericInsert := items[1].TextEdit.TextEdit.NewText
	expected = "fn(function*(${1}){${0}})"
	if genericInsert != expected {
		t.Errorf("generic insert text = %q, want %q", genericInsert, expected)
	}

	untracedInsert := items[2].TextEdit.TextEdit.NewText
	expected = "fnUntraced(function*(${1}){${0}})"
	if untracedInsert != expected {
		t.Errorf("untraced insert text = %q, want %q", untracedInsert, expected)
	}
}
