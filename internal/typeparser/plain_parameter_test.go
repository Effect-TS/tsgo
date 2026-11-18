package typeparser

import "testing"

func TestIsPlainIdentifierParameter(t *testing.T) {
	t.Parallel()

	_, _, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `
const plain = (value: number) => value
const optional = (value?: number) => value
const thisParameter = function (this: void) { return 1 }
const rest = (...values: Array<number>) => values
const defaulted = (value = 1) => value
const objectPattern = ({ value }: { value: number }) => value
const arrayPattern = ([value]: [number]) => value
`)
	defer done()

	for name, want := range map[string]bool{
		"plain":         true,
		"optional":      true,
		"thisParameter": true,
		"rest":          false,
		"defaulted":     false,
		"objectPattern": false,
		"arrayPattern":  false,
	} {
		parameter := findVariableInitializer(t, sf, name).Parameters()[0]
		if got := IsPlainIdentifierParameter(parameter); got != want {
			t.Fatalf("%s: IsPlainIdentifierParameter = %v, want %v", name, got, want)
		}
	}
	if IsPlainIdentifierParameter(nil) {
		t.Fatal("expected nil to be rejected")
	}
	if IsPlainIdentifierParameter(findVariableInitializer(t, sf, "plain")) {
		t.Fatal("expected a non-parameter node to be rejected")
	}
}
