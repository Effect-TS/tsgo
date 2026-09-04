package typeparser

import "testing"

func TestParseLazyExpressionFlags(t *testing.T) {
	t.Parallel()

	_, _, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `
const sync = () => 1
const asyncFunction = async () => 1
const generator = function* () { return 1 }
const parameter = (value: number) => value
`)
	defer done()

	if ParseLazyExpression(findVariableInitializer(t, sf, "sync"), LazyExpressionNone) == nil {
		t.Fatal("expected zero flags to accept a synchronous function")
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
}
