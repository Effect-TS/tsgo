package completions_test

import (
	"strings"
	"testing"
)

func TestContextSelfInClasses_NotInExtendsClause(t *testing.T) {
	t.Parallel()
	// Cursor in a variable declaration, not a class extends clause
	source := `import * as Context from "effect/Context"
const x = Context.Tag`
	items := contextSelfInClassesItems(t, source, len(source))
	if items != nil {
		t.Errorf("expected nil for cursor not in extends clause, got %d items", len(items))
	}
}

func TestContextSelfInClasses_NotInClass(t *testing.T) {
	t.Parallel()
	// Cursor after a standalone identifier that is not in any class
	source := `import * as Context from "effect/Context"
const x = Context.`
	items := contextSelfInClassesItems(t, source, len(source))
	if items != nil {
		t.Errorf("expected nil for cursor not in class, got %d items", len(items))
	}
}

func TestContextSelfInClasses_EmptySource(t *testing.T) {
	t.Parallel()
	source := ``
	items := contextSelfInClassesItems(t, source, 0)
	if items != nil {
		t.Errorf("expected nil for empty source, got %d items", len(items))
	}
}

func TestContextSelfInClasses_InsideImportDeclaration(t *testing.T) {
	t.Parallel()
	// Cursor inside an import declaration should not trigger completion
	source := `import { Context } from "effect"`
	pos := len(`import { Context`)
	items := contextSelfInClassesItems(t, source, pos)
	if items != nil {
		t.Errorf("expected nil for cursor inside import declaration, got %d items", len(items))
	}
}

func TestContextSelfInClasses_InterfaceNotClass(t *testing.T) {
	t.Parallel()
	// Interface extends clause, not a class
	source := `interface Foo extends Bar`
	items := contextSelfInClassesItems(t, source, len(source))
	if items != nil {
		t.Errorf("expected nil for interface extends (not class), got %d items", len(items))
	}
}

func TestContextSelfInClasses_AnonymousClass(t *testing.T) {
	t.Parallel()
	// Anonymous class has no name — should return nil
	source := `const x = class extends Context.Tag`
	items := contextSelfInClassesItems(t, source, len(source))
	if items != nil {
		t.Errorf("expected nil for anonymous class (no name), got %d items", len(items))
	}
}

func TestContextSelfInClasses_V4NamespaceImport(t *testing.T) {
	t.Parallel()

	source := `import * as Context from "effect/Context"

export class MyService extends Context.`
	items := contextSelfInClassesItemsWithPackageJSON(t, source, len(source))
	if len(items) != 2 {
		t.Fatalf("expected 2 items, got %d", len(items))
	}

	if items[0].Label != "Service<MyService, {}>" {
		t.Errorf("item[0].Label = %q, want %q", items[0].Label, "Service<MyService, {}>")
	}
	if got := items[0].TextEdit.TextEdit.NewText; !strings.HasPrefix(got, `Context.Service<MyService, {${0}}>()("`) || !strings.HasSuffix(got, `"){}`) {
		t.Errorf("item[0].insertText = %q", got)
	}

	if items[1].Label != "Service<MyService>({ make })" {
		t.Errorf("item[1].Label = %q, want %q", items[1].Label, "Service<MyService>({ make })")
	}
	if got := items[1].TextEdit.TextEdit.NewText; got != `Context.Service<MyService>()("@effect/harness-effect-v4/test/MyService", { make: ${0} }){}` {
		t.Errorf("item[1].insertText = %q", got)
	}
}

func TestContextSelfInClasses_V4DirectImport(t *testing.T) {
	t.Parallel()

	source := `import { Service } from "effect/Context"

export class MyService extends Service`
	items := contextSelfInClassesItemsWithPackageJSON(t, source, len(source))
	if len(items) != 2 {
		t.Fatalf("expected 2 items, got %d", len(items))
	}

	if got := items[0].TextEdit.TextEdit.NewText; !strings.HasPrefix(got, `Service<MyService, {${0}}>()("`) || !strings.HasSuffix(got, `"){}`) {
		t.Errorf("item[0].insertText = %q", got)
	}
	if got := items[1].TextEdit.TextEdit.NewText; got != `Service<MyService>()("@effect/harness-effect-v4/test/MyService", { make: ${0} }){}` {
		t.Errorf("item[1].insertText = %q", got)
	}
}

func TestContextSelfInClasses_V4IdentifierKeyPattern(t *testing.T) {
	t.Parallel()

	source := `// @test-config { "keyPatterns": [ { "pattern": "package-identifier", "target": "service" } ] }
import * as Context from "effect/Context"

export class MyService extends Context.`
	items := contextSelfInClassesItemsWithPackageJSON(t, source, len(source))
	if len(items) != 2 {
		t.Fatalf("expected 2 items, got %d", len(items))
	}

	for i, item := range items {
		if !strings.Contains(item.TextEdit.TextEdit.NewText, `"@effect/harness-effect-v4/test/MyService"`) {
			t.Errorf("item[%d].insertText = %q, want package identifier key", i, item.TextEdit.TextEdit.NewText)
		}
	}
}

func TestContextSelfInClasses_V4MiddleOfIdentifier(t *testing.T) {
	t.Parallel()

	source := `import { Effect, Stream } from "effect"
import * as Context from "effect/Context"

class Foo extends Context.S

Stream.unwrap(Effect.gen(function*() {
	const a = yield* Foo

	return Stream.succeed(a.count)
}))`
	position := strings.Index(source, "Context.S") + len("Context.S")
	items := contextSelfInClassesItemsWithPackageJSON(t, source, position)
	if len(items) != 2 {
		t.Fatalf("expected 2 items, got %d", len(items))
	}

	if items[0].Label != "Service<Foo, {}>" {
		t.Errorf("item[0].Label = %q, want %q", items[0].Label, "Service<Foo, {}>")
	}
	if items[1].Label != "Service<Foo>({ make })" {
		t.Errorf("item[1].Label = %q, want %q", items[1].Label, "Service<Foo>({ make })")
	}
	if got := items[0].TextEdit.TextEdit.NewText; !strings.HasPrefix(got, `Context.Service<Foo, {${0}}>()("`) || !strings.HasSuffix(got, `"){}`) {
		t.Errorf("item[0].insertText = %q", got)
	}
}
