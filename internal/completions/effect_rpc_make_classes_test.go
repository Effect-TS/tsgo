package completions

import (
	"testing"
)

func TestRpcMakeClasses_NotInExtendsClause(t *testing.T) {
	t.Parallel()
	// Cursor in a variable declaration, not a class extends clause
	source := `import * as Rpc from "@effect/rpc"
const x = Rpc.make`
	ctx := makeFnContext(source, len(source))

	items := runRpcMakeClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor not in extends clause, got %d items", len(items))
	}
}

func TestRpcMakeClasses_NotInClass(t *testing.T) {
	t.Parallel()
	// Cursor after a standalone identifier that is not in any class
	source := `const x = Rpc`
	ctx := makeFnContext(source, len(source))

	items := runRpcMakeClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor not in class, got %d items", len(items))
	}
}

func TestRpcMakeClasses_EmptySource(t *testing.T) {
	t.Parallel()
	source := ``
	ctx := makeFnContext(source, 0)

	items := runRpcMakeClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for empty source, got %d items", len(items))
	}
}

func TestRpcMakeClasses_InsideImportDeclaration(t *testing.T) {
	t.Parallel()
	// Cursor inside an import declaration should not trigger completion
	source := `import { Rpc } from "@effect/rpc"`
	pos := len(`import { Rpc`)
	ctx := makeFnContext(source, pos)

	items := runRpcMakeClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for cursor inside import declaration, got %d items", len(items))
	}
}

func TestRpcMakeClasses_InterfaceNotClass(t *testing.T) {
	t.Parallel()
	// Interface extends clause, not a class
	source := `interface Foo extends Bar`
	ctx := makeFnContext(source, len(source))

	items := runRpcMakeClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for interface extends (not class), got %d items", len(items))
	}
}

func TestRpcMakeClasses_AnonymousClass(t *testing.T) {
	t.Parallel()
	// Anonymous class has no name — should return nil
	source := `const x = class extends Rpc.make`
	ctx := makeFnContext(source, len(source))

	items := runRpcMakeClasses(ctx)
	if items != nil {
		t.Errorf("expected nil for anonymous class (no name), got %d items", len(items))
	}
}
