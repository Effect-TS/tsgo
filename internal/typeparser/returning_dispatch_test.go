package typeparser

import (
	"slices"
	"strings"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

func TestParseReturningDispatchAcceptedShapes(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name          string
		function      string
		tests         []string
		discriminants []string
		results       []string
		fallback      string
	}{
		{
			name:     "arrow conditional",
			function: `(value: unknown) => isA(value) ? first : fallback`,
			tests:    []string{"isA(value)"},
			results:  []string{"first"},
			fallback: "fallback",
		},
		{
			name: "returned conditional chain",
			function: `(value: unknown) => {
				return isA(value) ? first : isB(value) ? second : fallback
			}`,
			tests:    []string{"isA(value)", "isB(value)"},
			results:  []string{"first", "second"},
			fallback: "fallback",
		},
		{
			name: "function expression if with trailing fallback",
			function: `function(value: unknown) {
				if (isA(value)) return first
				return fallback
			}`,
			tests:    []string{"isA(value)"},
			results:  []string{"first"},
			fallback: "fallback",
		},
		{
			name: "switch exposes discriminant and case expressions",
			function: `(value: unknown) => {
				switch (classify(value)) {
					case "A": return first
					case "B": return second
					default: return fallback
				}
			}`,
			tests:         []string{`"A"`, `"B"`},
			discriminants: []string{"classify(value)", "classify(value)"},
			results:       []string{"first", "second"},
			fallback:      "fallback",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			sf, function := parseReturningDispatchTestFunction(t, tt.function)
			dispatch := ParseReturningDispatch(function)
			if dispatch == nil {
				t.Fatal("expected returning dispatch")
			}
			if dispatch.Node != function || dispatch.Body == nil || len(dispatch.Params) != 1 || dispatch.Dispatch == nil {
				t.Fatalf("unexpected function evidence: %+v", dispatch)
			}
			resultDispatch := dispatch.Dispatch
			if resultDispatch.Node != dispatch.Body {
				t.Fatalf("result dispatch node does not preserve the function body")
			}

			tests := make([]string, len(resultDispatch.Branches))
			discriminants := make([]string, len(resultDispatch.Branches))
			results := make([]string, len(resultDispatch.Branches))
			for index, branch := range resultDispatch.Branches {
				test := branch.Condition.Subject
				if branch.Condition.Kind == DispatchConditionSwitchCase {
					discriminants[index] = returningDispatchNodeText(sf, branch.Condition.Subject)
					test = branch.Condition.Value
				}
				tests[index] = returningDispatchNodeText(sf, test)
				results[index] = returningDispatchNodeText(sf, branch.Result)
				if branch.Condition.Source == nil {
					t.Fatalf("branch %d is missing its source test node", index)
				}
			}
			if !slices.Equal(tests, tt.tests) {
				t.Fatalf("tests = %v, want %v", tests, tt.tests)
			}
			if !slices.Equal(results, tt.results) {
				t.Fatalf("results = %v, want %v", results, tt.results)
			}
			if len(tt.discriminants) > 0 && !slices.Equal(discriminants, tt.discriminants) {
				t.Fatalf("discriminants = %v, want %v", discriminants, tt.discriminants)
			}
			if got := returningDispatchNodeText(sf, resultDispatch.Fallback); got != tt.fallback {
				t.Fatalf("fallback = %q, want %q", got, tt.fallback)
			}
		})
	}
}

func TestParseResultDispatchInputContract(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name       string
		expression string
		branches   []string
		fallback   string
		accepted   bool
	}{
		{
			name:       "conditional expression",
			expression: `isA(value) ? first : fallback`,
			branches:   []string{"isA(value)"},
			fallback:   "fallback",
			accepted:   true,
		},
		{
			name:       "false branch chain is flattened",
			expression: `isA(value) ? first : isB(value) ? second : fallback`,
			branches:   []string{"isA(value)", "isB(value)"},
			fallback:   "fallback",
			accepted:   true,
		},
		{
			name:       "true branch nesting is rejected",
			expression: `isA(value) ? (isB(value) ? first : second) : fallback`,
			accepted:   false,
		},
		{
			name:       "plain expression is rejected",
			expression: `first`,
			accepted:   false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			sf, function := parseReturningDispatchTestFunction(t, `() => `+tt.expression)
			body := GetFunctionLikeBody(function)
			dispatch := ParseResultDispatch(body)
			if !tt.accepted {
				if dispatch != nil {
					t.Fatalf("expected parse rejection, got %+v", dispatch)
				}
				return
			}
			if dispatch == nil || dispatch.Node != body {
				t.Fatalf("expected result dispatch for body, got %+v", dispatch)
			}
			conditions := make([]string, len(dispatch.Branches))
			for index, branch := range dispatch.Branches {
				if branch.Condition.Kind != DispatchConditionPredicate || branch.Condition.Subject == nil || branch.Condition.Value != nil {
					t.Fatalf("branch %d is not a predicate", index)
				}
				conditions[index] = returningDispatchNodeText(sf, branch.Condition.Subject)
			}
			if !slices.Equal(conditions, tt.branches) {
				t.Fatalf("branches = %v, want %v", conditions, tt.branches)
			}
			if got := returningDispatchNodeText(sf, dispatch.Fallback); got != tt.fallback {
				t.Fatalf("fallback = %q, want %q", got, tt.fallback)
			}
		})
	}
}

func TestParseResultDispatchTagHints(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name        string
		function    string
		tagSubjects []string
		tagValues   []string
	}{
		{
			name:        "tag equality predicate",
			function:    `(value: unknown) => value._tag === "Some" ? first : fallback`,
			tagSubjects: []string{"value"},
			tagValues:   []string{`"Some"`},
		},
		{
			name:        "reversed tag equality predicate",
			function:    `(error: unknown) => "ReasonA" == error.reason._tag ? first : fallback`,
			tagSubjects: []string{"error.reason"},
			tagValues:   []string{`"ReasonA"`},
		},
		{
			name: "tag switch cases",
			function: `(value: unknown) => {
				switch (value._tag) {
					case "Some": return first
					case "None": return second
					default: return fallback
				}
			}`,
			tagSubjects: []string{"value", "value"},
			tagValues:   []string{`"Some"`, `"None"`},
		},
		{
			name:        "non-literal tag value remains source evidence",
			function:    `(value: unknown) => value._tag === expectedTag ? first : fallback`,
			tagSubjects: []string{"value"},
			tagValues:   []string{"expectedTag"},
		},
		{
			name:        "non-tag predicate has no hints",
			function:    `(value: unknown) => value.kind === "Some" ? first : fallback`,
			tagSubjects: []string{""},
			tagValues:   []string{""},
		},
		{
			name:        "non-equality tag predicate has no hints",
			function:    `(value: unknown) => value._tag !== "Some" ? first : fallback`,
			tagSubjects: []string{""},
			tagValues:   []string{""},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			sf, function := parseReturningDispatchTestFunction(t, tt.function)
			parsed := ParseReturningDispatch(function)
			if parsed == nil || parsed.Dispatch == nil {
				t.Fatal("expected returning dispatch")
			}
			if len(parsed.Dispatch.Branches) != len(tt.tagSubjects) || len(tt.tagSubjects) != len(tt.tagValues) {
				t.Fatalf("branches = %d, want %d", len(parsed.Dispatch.Branches), len(tt.tagSubjects))
			}
			for index, branch := range parsed.Dispatch.Branches {
				if got := returningDispatchNodeText(sf, branch.Condition.TagSubject); got != tt.tagSubjects[index] {
					t.Fatalf("branch %d tag subject = %q, want %q", index, got, tt.tagSubjects[index])
				}
				if got := returningDispatchNodeText(sf, branch.Condition.TagValue); got != tt.tagValues[index] {
					t.Fatalf("branch %d tag value = %q, want %q", index, got, tt.tagValues[index])
				}
			}
		})
	}
}

func TestResultDispatchCommonTagSubject(t *testing.T) {
	t.Parallel()
	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `
		class ReasonA { readonly _tag = "ReasonA" }
		class ReasonB { readonly _tag = "ReasonB" }
		class Wrapper {
			constructor(
				readonly reason: ReasonA | ReasonB,
				readonly alternateReason: ReasonA | ReasonB
			) {}
		}
		declare const other: Wrapper

		const sameIdentifier = (reason: ReasonA | ReasonB) =>
			reason._tag === "ReasonA" ? 1 : reason._tag === "ReasonB" ? 2 : 0

		const samePropertyChain = (error: Wrapper) => {
			if (error.reason._tag === "ReasonA") return 1
			if (error.reason._tag === "ReasonB") return 2
			return 0
		}

		const sameSwitchSubject = (error: Wrapper) => {
			switch (error.reason._tag) {
				case "ReasonA": return 1
				case "ReasonB": return 2
				default: return 0
			}
		}

		const differentRoot = (error: Wrapper) => {
			if (error.reason._tag === "ReasonA") return 1
			if (other.reason._tag === "ReasonB") return 2
			return 0
		}

		const differentPropertyChain = (error: Wrapper) => {
			if (error.reason._tag === "ReasonA") return 1
			if (error.alternateReason._tag === "ReasonB") return 2
			return 0
		}

		const partlyUntagged = (error: Wrapper) => {
			if (error.reason._tag === "ReasonA") return 1
			if (Boolean(error.reason)) return 2
			return 0
		}
	`)
	defer done()

	tests := []struct {
		name    string
		subject string
	}{
		{name: "sameIdentifier", subject: "reason"},
		{name: "samePropertyChain", subject: "error.reason"},
		{name: "sameSwitchSubject", subject: "error.reason"},
		{name: "differentRoot"},
		{name: "differentPropertyChain"},
		{name: "partlyUntagged"},
	}
	for _, tt := range tests {
		function := findReturningDispatchTestFunction(t, sf, tt.name)
		parsed := ParseReturningDispatch(function)
		if parsed == nil || parsed.Dispatch == nil {
			t.Fatalf("%s: expected returning dispatch", tt.name)
		}
		subject := parsed.Dispatch.CommonTagSubject(tp)
		if got := returningDispatchNodeText(sf, subject); got != tt.subject {
			t.Fatalf("%s: common tag subject = %q, want %q", tt.name, got, tt.subject)
		}
	}
}

func TestParseReturningDispatchInputContract(t *testing.T) {
	t.Parallel()
	sf := parseSource(`
		function declaration(value: unknown) {
			return isA(value) ? first : fallback
		}
		const bodyOnly = (value: unknown) => isA(value) ? first : fallback
		const plain = (value: unknown) => first
		const generic = <A>(value: A) => isA(value) ? first : fallback
	`)

	var declaration *ast.Node
	var body *ast.Node
	var plain *ast.Node
	var generic *ast.Node
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		switch GetFunctionLikeName(node) {
		case "declaration":
			declaration = node
		case "bodyOnly":
			body = GetFunctionLikeBody(node)
		case "plain":
			plain = node
		case "generic":
			generic = node
		}
		return node.ForEachChild(walk)
	}
	walk(sf.AsNode())

	for name, node := range map[string]*ast.Node{
		"function declaration": declaration,
		"body node":            body,
		"plain function":       plain,
		"generic function":     generic,
	} {
		if node == nil {
			t.Fatalf("%s fixture was not found", name)
		}
		if dispatch := ParseReturningDispatch(node); dispatch != nil {
			t.Fatalf("%s: expected parse rejection, got %+v", name, dispatch)
		}
	}
}

func findReturningDispatchTestFunction(t *testing.T, sf *ast.SourceFile, name string) *ast.Node {
	t.Helper()
	var function *ast.Node
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if (node.Kind == ast.KindArrowFunction || node.Kind == ast.KindFunctionExpression) && GetFunctionLikeName(node) == name {
			function = node
			return true
		}
		return node.ForEachChild(walk)
	}
	walk(sf.AsNode())
	if function == nil {
		t.Fatalf("function %s not found", name)
	}
	return function
}

func parseReturningDispatchTestFunction(t *testing.T, functionSource string) (*ast.SourceFile, *ast.Node) {
	t.Helper()
	sf := parseSource("const decode = " + functionSource)
	var function *ast.Node
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindArrowFunction || node.Kind == ast.KindFunctionExpression {
			function = node
			return true
		}
		return node.ForEachChild(walk)
	}
	walk(sf.AsNode())
	if function == nil {
		t.Fatal("failed to parse returning function")
	}
	return sf, function
}

func returningDispatchNodeText(sf *ast.SourceFile, node *ast.Node) string {
	if sf == nil || node == nil {
		return ""
	}
	return strings.TrimSpace(scanner.GetSourceTextOfNodeFromSourceFile(sf, node, false))
}
