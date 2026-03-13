package completions

import (
	"testing"
)

func TestEffectSqlModelSelfInClasses_NotInExtendsClause(t *testing.T) {
	t.Parallel()
	// Cursor in a variable declaration, not a class extends clause
	source := `import * as Model from "@effect/sql/Model"
const x = Model.Class`
	ctx := makeFnContext(source, len(source))

	items := runEffectSqlModelSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor not in extends clause, got %d items", len(items))
	}
}

func TestEffectSqlModelSelfInClasses_NotInClass(t *testing.T) {
	t.Parallel()
	// Cursor after a standalone identifier that is not in any class
	source := `const x = Model`
	ctx := makeFnContext(source, len(source))

	items := runEffectSqlModelSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor not in class, got %d items", len(items))
	}
}

func TestEffectSqlModelSelfInClasses_EmptySource(t *testing.T) {
	t.Parallel()
	source := ``
	ctx := makeFnContext(source, 0)

	items := runEffectSqlModelSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for empty source, got %d items", len(items))
	}
}

func TestEffectSqlModelSelfInClasses_InsideImportDeclaration(t *testing.T) {
	t.Parallel()
	// Cursor inside an import declaration should not trigger completion
	source := `import { Model } from "@effect/sql"`
	pos := len(`import { Model`)
	ctx := makeFnContext(source, pos)

	items := runEffectSqlModelSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor inside import declaration, got %d items", len(items))
	}
}

func TestEffectSqlModelSelfInClasses_InterfaceNotClass(t *testing.T) {
	t.Parallel()
	// Interface extends clause, not a class
	source := `interface Foo extends Bar`
	ctx := makeFnContext(source, len(source))

	items := runEffectSqlModelSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for interface extends (not class), got %d items", len(items))
	}
}

func TestEffectSqlModelSelfInClasses_AnonymousClass(t *testing.T) {
	t.Parallel()
	// Anonymous class has no name — should return nil
	source := `const x = class extends Model.Class`
	ctx := makeFnContext(source, len(source))

	items := runEffectSqlModelSelfInClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for anonymous class (no name), got %d items", len(items))
	}
}
