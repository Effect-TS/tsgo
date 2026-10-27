package typeparser

import "testing"

func TestIsExpressionValueStableAtLocation(t *testing.T) {
	t.Parallel()

	source := `
enum Phase { Done = "done" }
declare const ambient: string
declare let ambientLet: string
const early = "early"
const plain = { value: 1 }
let mutable = 1
let PhaseAlias = Phase

const negative = -1
const negativeBig = -1n
const falseLiteral = false
const undefinedRef = undefined
const enumMember = Phase.Done
const ambientConst = ambient
const earlyConst = early
function nested() {
  const nestedRef = early
  return nestedRef
}

const positive = +1
const negatedIdentifier = -mutable
const sum = 1 + 1
const mutableRef = mutable
const ambientLetRef = ambientLet
const aliasedEnum = PhaseAlias.Done
const propertyRead = plain.value
function deferred() {
  const deferredRef = later
  return deferredRef
}
const laterEnumRef = LaterEnum.A
const laterAmbientRef = laterAmbient
const later = "later"
enum LaterEnum { A }
declare const laterAmbient: string
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done()

	tests := []struct {
		name   string
		stable bool
	}{
		{"negative", true},
		{"negativeBig", true},
		{"falseLiteral", true},
		{"undefinedRef", true},
		{"enumMember", true},
		{"ambientConst", true},
		{"earlyConst", true},
		{"nestedRef", true},
		{"positive", false},
		{"negatedIdentifier", false},
		{"sum", false},
		{"mutableRef", false},
		{"ambientLetRef", false},
		{"aliasedEnum", false},
		{"propertyRead", false},
		{"deferredRef", false},
		{"laterEnumRef", false},
		{"laterAmbientRef", false},
	}

	for _, test := range tests {
		expression := findVariableInitializer(t, sf, test.name)
		if expression == nil {
			t.Fatalf("missing initializer for %s", test.name)
		}
		if got := tp.IsExpressionValueStableAtLocation(expression, expression); got != test.stable {
			t.Errorf("IsExpressionValueStableAtLocation(%s) = %v, want %v", test.name, got, test.stable)
		}
	}
}
