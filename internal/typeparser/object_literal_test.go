package typeparser

import (
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

func TestObjectLiteralProperty(t *testing.T) {
	t.Parallel()
	const (
		declared      = ObjectLiteralPropertyDeclared
		notAssignment = ObjectLiteralPropertyNotAssignment
		spread        = ObjectLiteralPropertyMaybeSpread
		computed      = ObjectLiteralPropertyMaybeComputed
	)
	cases := []struct {
		name, expression string
		wantInitializer  string
		wantFlags        ObjectLiteralPropertyFlags
	}{
		{"absent", `{ discard: true }`, "", ObjectLiteralPropertyNone},
		{"empty", `{}`, "", ObjectLiteralPropertyNone},
		{"assignment", `{ concurrency: 4 }`, "4", declared},
		{"string key", `{ "concurrency": 4 }`, "4", declared},
		{"computed string key", `{ ["concurrency"]: 4 }`, "4", declared},
		{"computed template key", "{ [`concurrency`]: 4 }", "4", declared},
		{"last assignment wins", `{ concurrency: 1, concurrency: 2 }`, "2", declared},
		{"under outer expressions", `({ concurrency: 4 } as const)`, "4", declared},
		{"under satisfies", `({ concurrency: 4 }) satisfies object`, "4", declared},
		{"shorthand", `{ concurrency }`, "", declared | notAssignment},
		{"method", `{ concurrency() { return 1 } }`, "", declared | notAssignment},
		{"assignment after shorthand", `{ concurrency, concurrency: 4 }`, "4", declared},
		{"spread only", `{ ...options }`, "", spread},
		{"spread after", `{ concurrency: 4, ...options }`, "", declared | spread},
		{"spread before", `{ ...options, concurrency: 4 }`, "4", declared},
		{"computed only", `{ [key]: 4 }`, "", computed},
		{"computed after", `{ concurrency: 4, [key]: 1 }`, "", declared | computed},
		{"computed before", `{ [key]: 1, concurrency: 4 }`, "4", declared},
		{"spread and computed", `{ ...options, [key]: 1 }`, "", spread | computed},
		{"not an object literal", `options`, "", ObjectLiteralPropertyNotObjectLiteral},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			t.Parallel()
			sf := parseSource("declare const options: object, key: string, concurrency: number\nconst x = " + c.expression)
			statements := sf.Statements.Nodes
			declaration := statements[len(statements)-1].AsVariableStatement().DeclarationList.AsVariableDeclarationList().Declarations.Nodes[0]
			initializer, flags := ObjectLiteralProperty(declaration.Initializer(), "concurrency")
			got := ""
			if initializer != nil {
				got = scanner.GetTextOfNode(initializer)
			}
			if got != c.wantInitializer || flags != c.wantFlags {
				t.Errorf("ObjectLiteralProperty = (%q, %d), want (%q, %d)", got, flags, c.wantInitializer, c.wantFlags)
			}
			if (initializer != nil) != (flags == ObjectLiteralPropertyDeclared) {
				t.Errorf("initializer %q with flags %d: an initializer must come with Declared alone", got, flags)
			}
			if wrapper := ObjectLiteralPropertyInitializer(declaration.Initializer(), "concurrency"); wrapper != initializer {
				t.Errorf("ObjectLiteralPropertyInitializer = %v, want %v", wrapper, initializer)
			}
		})
	}
	if initializer, flags := ObjectLiteralProperty((*ast.Node)(nil), "concurrency"); initializer != nil || flags != ObjectLiteralPropertyNotObjectLiteral {
		t.Errorf("ObjectLiteralProperty(nil) = (%v, %d), want (nil, NotObjectLiteral)", initializer, flags)
	}
}
