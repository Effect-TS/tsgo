package completions

import (
	"testing"
)

func TestContextSelfInClasses_NotInExtendsClause(t *testing.T) {
	t.Parallel()
	// Cursor in a variable declaration, not a class extends clause
	source := `import * as Context from "effect/Context"
const x = Context.Tag`
	ctx := makeFnContext(source, len(source))

	items := runContextSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor not in extends clause, got %d items", len(items))
	}
}

func TestContextSelfInClasses_NotInClass(t *testing.T) {
	t.Parallel()
	// Cursor after a standalone identifier that is not in any class
	source := `const x = Context`
	ctx := makeFnContext(source, len(source))

	items := runContextSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor not in class, got %d items", len(items))
	}
}

func TestContextSelfInClasses_EmptySource(t *testing.T) {
	t.Parallel()
	source := ``
	ctx := makeFnContext(source, 0)

	items := runContextSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for empty source, got %d items", len(items))
	}
}

func TestContextSelfInClasses_InsideImportDeclaration(t *testing.T) {
	t.Parallel()
	// Cursor inside an import declaration should not trigger completion
	source := `import { Context } from "effect"`
	pos := len(`import { Context`)
	ctx := makeFnContext(source, pos)

	items := runContextSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor inside import declaration, got %d items", len(items))
	}
}

func TestContextSelfInClasses_InterfaceNotClass(t *testing.T) {
	t.Parallel()
	// Interface extends clause, not a class
	source := `interface Foo extends Bar`
	ctx := makeFnContext(source, len(source))

	items := runContextSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for interface extends (not class), got %d items", len(items))
	}
}

func TestContextSelfInClasses_AnonymousClass(t *testing.T) {
	t.Parallel()
	// Anonymous class has no name — should return nil
	source := `const x = class extends Context.Tag`
	ctx := makeFnContext(source, len(source))

	items := runContextSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for anonymous class (no name), got %d items", len(items))
	}
}
