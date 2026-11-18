package typeparser

import "testing"

func TestParseLazyExpressionMultiStatementBlock(t *testing.T) {
	t.Parallel()
	_, _, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `
const statements = () => { const value = 1; console.log(value); return value }
const nested = () => { const inner = () => { return 2 }; return inner() }
const early = () => { if (Math.random()) return 1; return 2 }
const bare = () => { if (Math.random()) return; return 2 }
const finallyReturn = () => { try { return 1 } finally {} return 2 }
const notFinal = () => { return 1; console.log(2) }
const empty = () => {}
const single = () => { return 3 }
`)
	defer done()
	for _, name := range []string{"statements", "nested", "single"} {
		parsed := ParseLazyExpression(findVariableInitializer(t, sf, name), LazyExpressionAllowMultiStatementBlock)
		if parsed == nil {
			t.Fatalf("%s: expected the final return expression", name)
		}
		text := sf.Text()[parsed.Expression().Pos():parsed.Expression().End()]
		want := map[string]string{"statements": " value", "nested": " inner()", "single": " 3"}[name]
		if text != want {
			t.Fatalf("%s: expression = %q, want %q", name, text, want)
		}
	}
	for _, name := range []string{"early", "bare", "finallyReturn", "notFinal", "empty"} {
		if ParseLazyExpression(findVariableInitializer(t, sf, name), LazyExpressionAllowMultiStatementBlock) != nil {
			t.Fatalf("%s: expected rejection", name)
		}
	}
	if ParseLazyExpression(findVariableInitializer(t, sf, "statements"), LazyExpressionNone) != nil {
		t.Fatal("the default parser must still reject multiple statements")
	}
}

func TestParseLazyExpressionFlags(t *testing.T) {
	t.Parallel()

	_, _, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `
const sync = () => 1
const parenthesized = (((() => 1)))
const asyncFunction = async () => 1
const generator = function* () { return 1 }
const parameter = (value: number) => value
const genericArrow = <A,>(value: A) => value
const genericFunction = function <A>(value: A) { return value }
const two = (a: number, b: number) => a + b
const thisAndValue = function (this: void, value: number) { return value }
`)
	defer done()

	if ParseLazyExpression(findVariableInitializer(t, sf, "sync"), LazyExpressionNone) == nil {
		t.Fatal("expected zero flags to accept a synchronous function")
	}
	if ParseLazyExpression(findVariableInitializer(t, sf, "parenthesized"), LazyExpressionNone) == nil {
		t.Fatal("expected the parser to skip parentheses")
	}
	asyncFunction := findVariableInitializer(t, sf, "asyncFunction")
	if ParseLazyExpression(asyncFunction, LazyExpressionNone) != nil {
		t.Fatal("expected zero flags to reject an async function")
	}
	if ParseLazyExpression(asyncFunction, LazyExpressionAllowAsync) == nil {
		t.Fatal("expected AllowAsync to accept an async function")
	}
	generator := findVariableInitializer(t, sf, "generator")
	if ParseLazyExpression(generator, LazyExpressionNone) != nil {
		t.Fatal("expected zero flags to reject a generator function")
	}
	if ParseLazyExpression(generator, LazyExpressionAllowGenerator) == nil {
		t.Fatal("expected AllowGenerator to accept a generator function")
	}
	if ParseLazyExpression(findVariableInitializer(t, sf, "parameter"), LazyExpressionThunk) != nil {
		t.Fatal("expected Thunk to reject a function with parameters")
	}
	for _, name := range []string{"genericArrow", "genericFunction"} {
		generic := findVariableInitializer(t, sf, name)
		if ParseLazyExpression(generic, LazyExpressionNone) != nil {
			t.Fatalf("%s: expected zero flags to reject a generic function", name)
		}
		if ParseLazyExpression(generic, LazyExpressionAllowTypeParameters) == nil {
			t.Fatalf("%s: expected AllowTypeParameters to accept a generic function", name)
		}
	}
	for name, want := range map[string]map[LazyExpressionFlags]bool{
		"sync":         {LazyExpressionAtMostOneParameter: true, LazyExpressionAtLeastOneParameter: false},
		"parameter":    {LazyExpressionAtMostOneParameter: true, LazyExpressionAtLeastOneParameter: true},
		"two":          {LazyExpressionAtMostOneParameter: false, LazyExpressionAtLeastOneParameter: true},
		"thisAndValue": {LazyExpressionAtMostOneParameter: false, LazyExpressionAtLeastOneParameter: true},
	} {
		for flags, accepted := range want {
			if got := ParseLazyExpression(findVariableInitializer(t, sf, name), flags) != nil; got != accepted {
				t.Fatalf("%s with flags %d: accepted = %v, want %v", name, flags, got, accepted)
			}
		}
	}
}
