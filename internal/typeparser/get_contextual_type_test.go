package typeparser

import (
	"context"
	"fmt"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

func TestGetContextualTypeInGeneratorExpressions(t *testing.T) {
	t.Parallel()

	const literal = `const options: Options = { value: 42 };`
	for _, test := range []struct {
		name   string
		source string
		skip   bool
	}{
		{"inferred generator", `const gen = function* () { ` + literal + ` };`, true},
		{"named generator", `const gen = function* named() { ` + literal + ` };`, true},
		{"async generator", `const gen = async function* () { ` + literal + ` };`, true},
		{"contextually typed generator", `const gen: () => Generator<never, void, unknown> = function* () { ` + literal + ` };`, true},
		{"parameter default", `const gen = function* (arg = (() => { ` + literal + ` return options; })()) {};`, true},
		{"nested function", `const gen = function* () { function inner() { ` + literal + ` } };`, true},
		{"nested arrow", `const gen = function* () { const inner = () => { ` + literal + ` }; };`, true},
		{"annotated inner generator", `const gen = function* () { const inner = function* (): Generator<never, void, unknown> { ` + literal + ` }; };`, true},
		{"annotated generator", `const gen = function* (): Generator<never, void, unknown> { ` + literal + ` };`, false},
		{"generator declaration", `function* gen() { ` + literal + ` }`, false},
		{"generator method", `class C { *gen() { ` + literal + ` } }`, false},
		{"ordinary function", `const gen = function () { ` + literal + ` };`, false},
		{"sibling generator", `declare function accept(a: Options, b: () => Generator<never, void, unknown>): void; accept((() => { ` + literal + ` return options; })(), function* () {});`, false},
	} {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			c, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `interface Options { value: number }; `+test.source)
			defer done()
			node := findIdentifierByText(t, sf, "options", 0).Parent.Initializer()
			for _, flags := range []checker.ContextFlags{checker.ContextFlagsNone, checker.ContextFlagsNoConstraints, checker.ContextFlagsIgnoreNodeInferences, checker.ContextFlagsIgnoreNodeInferences | checker.ContextFlagsNoConstraints} {
				got := tp.GetContextualType(node, flags)
				if test.skip && flags&checker.ContextFlagsIgnoreNodeInferences != 0 {
					if got != nil {
						t.Errorf("flags %d: got %s, want unavailable contextual evidence", flags, c.TypeToString(got))
					}
				} else if got == nil {
					t.Errorf("flags %d: contextual type is nil, want Options", flags)
				} else if name := c.TypeToString(got); name != "Options" {
					t.Errorf("flags %d: contextual type = %q, want Options", flags, name)
				}
			}
		})
	}
}

func TestGetContextualTypeOnGeneratorExpression(t *testing.T) {
	t.Parallel()
	c, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `declare function accept(body: () => Generator<number, void, unknown>): void;
accept(function* () { yield 1; });`)
	defer done()
	node := sf.Statements.Nodes[1].Expression().Arguments()[0]
	if got := tp.GetContextualType(node, checker.ContextFlagsNone); got == nil || c.TypeToString(got) != "() => Generator<number, void, unknown>" {
		t.Fatalf("normal generator context = %v, want () => Generator<number, void, unknown>", got)
	}
	if got := tp.GetContextualType(node, checker.ContextFlagsIgnoreNodeInferences); got != nil {
		t.Fatalf("ignored-inference generator context = %s, want nil", c.TypeToString(got))
	}
}

func TestInferredGeneratorContextualCaches(t *testing.T) {
	t.Parallel()
	for _, constraintFirst := range []bool{false, true} {
		t.Run(fmt.Sprintf("constraintFirst=%t", constraintFirst), func(t *testing.T) {
			t.Parallel()
			c, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `interface Options { value: number }
const gen = function* () { const options: Options = { value: 42 }; };`)
			defer done()
			node := findIdentifierByText(t, sf, "options", 0).Parent.Initializer()
			if constraintFirst {
				if got := tp.CachedObjectLiteralConstraintContextualType(node); got != nil {
					t.Fatalf("constraint context = %s, want nil", c.TypeToString(got))
				}
			}
			if got := tp.CachedObjectLiteralContextualType(node); got == nil || c.TypeToString(got) != "Options" {
				t.Fatalf("normal context = %v, want Options", got)
			}
			if got := tp.CachedObjectLiteralConstraintContextualType(node); got != nil {
				t.Fatalf("constraint context = %s, want nil", c.TypeToString(got))
			}
			if entry := tp.links.ObjectLiteralConstraintContextualType.TryGet(node); entry == nil || *entry != nil {
				t.Fatal("skipped constraint lookup must cache a present nil")
			}
			_ = c.GetDiagnostics(context.Background(), sf)
			other := NewTypeParser(c.Program(), c)
			if other.links.ObjectLiteralConstraintContextualType.TryGet(node) != tp.links.ObjectLiteralConstraintContextualType.TryGet(node) {
				t.Fatal("parsers over one checker must share the negative cache entry")
			}
			if got := other.CachedObjectLiteralContextualType(node); got == nil || c.TypeToString(got) != "Options" {
				t.Fatalf("shared normal context = %v, want Options", got)
			}
			if got := other.CachedObjectLiteralConstraintContextualType(node); got != nil {
				t.Fatalf("shared constraint context after checking = %s, want nil", c.TypeToString(got))
			}
		})
	}
}
