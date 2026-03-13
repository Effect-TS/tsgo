package completions

import (
	"testing"
)

func TestEffectSelfInClasses_NotInExtendsClause(t *testing.T) {
	t.Parallel()
	// Cursor in a variable declaration, not a class extends clause
	source := `import * as Effect from "effect/Effect"
const x = Effect.Service`
	ctx := makeFnContext(source, len(source))

	items := runEffectSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor not in extends clause, got %d items", len(items))
	}
}

func TestEffectSelfInClasses_NotInClass(t *testing.T) {
	t.Parallel()
	// Cursor after a standalone identifier that is not in any class
	source := `const x = Effect`
	ctx := makeFnContext(source, len(source))

	items := runEffectSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor not in class, got %d items", len(items))
	}
}

func TestEffectSelfInClasses_EmptySource(t *testing.T) {
	t.Parallel()
	source := ``
	ctx := makeFnContext(source, 0)

	items := runEffectSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for empty source, got %d items", len(items))
	}
}

func TestEffectSelfInClasses_InsideImportDeclaration(t *testing.T) {
	t.Parallel()
	// Cursor inside an import declaration should not trigger completion
	source := `import { Effect } from "effect"`
	pos := len(`import { Effect`)
	ctx := makeFnContext(source, pos)

	items := runEffectSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor inside import declaration, got %d items", len(items))
	}
}

func TestEffectSelfInClasses_InterfaceNotClass(t *testing.T) {
	t.Parallel()
	// Interface extends clause, not a class
	source := `interface Foo extends Bar`
	ctx := makeFnContext(source, len(source))

	items := runEffectSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for interface extends (not class), got %d items", len(items))
	}
}

func TestEffectSelfInClasses_AnonymousClass(t *testing.T) {
	t.Parallel()
	// Anonymous class has no name — should return nil
	source := `const x = class extends Effect.Service`
	ctx := makeFnContext(source, len(source))

	items := runEffectSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for anonymous class (no name), got %d items", len(items))
	}
}
