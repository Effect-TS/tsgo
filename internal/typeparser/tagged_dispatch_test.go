package typeparser

import (
	"slices"
	"strings"
	"testing"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/scanner"
)

func TestParseTaggedDispatchAcceptedShapes(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name     string
		body     string
		tags     []string
		results  []string
		fallback string
	}{
		{
			name: "switch preserves source order and fallback",
			body: `{
				switch (value._tag) {
					case "B": return second
					case "A": return first
					default: return fallback
				}
			}`,
			tags:     []string{"B", "A"},
			results:  []string{"second", "first"},
			fallback: "fallback",
		},
		{
			name: "switch fallback is optional",
			body: `{
				switch (value._tag) {
					case "A": return first
				}
			}`,
			tags:    []string{"A"},
			results: []string{"first"},
		},
		{
			name:     "right nested conditional supports reversed and loose equality",
			body:     `value._tag == "A" ? first : "B" === value._tag ? second : fallback`,
			tags:     []string{"A", "B"},
			results:  []string{"first", "second"},
			fallback: "fallback",
		},
		{
			name: "if else-if preserves source order and fallback",
			body: `{
				if (value._tag === "A") return first
				else if ("B" == value._tag) { return second }
				else return fallback
			}`,
			tags:     []string{"A", "B"},
			results:  []string{"first", "second"},
			fallback: "fallback",
		},
		{
			name: "sequential ifs share the ordered model",
			body: `{
				if (value._tag === "A") return first
				if (value._tag === "B") return second
				return fallback
			}`,
			tags:     []string{"A", "B"},
			results:  []string{"first", "second"},
			fallback: "fallback",
		},
		{
			name: "if fallback is optional",
			body: `{
				if (value._tag === "A") return first
			}`,
			tags:    []string{"A"},
			results: []string{"first"},
		},
		{
			name:     "returned conditional is decoded from a block",
			body:     `{ return value._tag === "A" ? first : fallback }`,
			tags:     []string{"A"},
			results:  []string{"first"},
			fallback: "fallback",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			sf, body := parseTaggedDispatchTestBody(t, tt.body)
			dispatch := parseTaggedDispatchSyntax(body)
			if dispatch == nil {
				t.Fatal("expected tagged dispatch")
			}

			tags := make([]string, len(dispatch.Branches))
			results := make([]string, len(dispatch.Branches))
			for index, branch := range dispatch.Branches {
				tags[index] = branch.Tag
				results[index] = taggedDispatchNodeText(sf, branch.Result)
				if branch.TagNode == nil || branch.TestNode == nil || branch.Discriminant == nil {
					t.Fatalf("branch %d is missing source evidence", index)
				}
				if got := taggedDispatchNodeText(sf, branch.Discriminant); got != "value._tag" {
					t.Fatalf("branch %d discriminant = %q, want value._tag", index, got)
				}
			}
			if !slices.Equal(tags, tt.tags) {
				t.Fatalf("tags = %v, want %v", tags, tt.tags)
			}
			if !slices.Equal(results, tt.results) {
				t.Fatalf("results = %v, want %v", results, tt.results)
			}
			if got := taggedDispatchNodeText(sf, dispatch.Fallback); got != tt.fallback {
				t.Fatalf("fallback = %q, want %q", got, tt.fallback)
			}
		})
	}
}

func TestParseTaggedDispatchRejectedShapes(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name string
		body string
	}{
		{
			name: "duplicate switch tag",
			body: `{ switch (value._tag) {
				case "A": return first
				case "A": return second
			} }`,
		},
		{
			name: "duplicate conditional tag",
			body: `value._tag === "A" ? first : value._tag === "A" ? second : fallback`,
		},
		{
			name: "duplicate if tag",
			body: `{ if (value._tag === "A") return first
				else if (value._tag === "A") return second
				else return fallback }`,
		},
		{
			name: "grouped switch labels fall through",
			body: `{ switch (value._tag) {
				case "A":
				case "B": return result
			} }`,
		},
		{
			name: "switch break does not yield a result",
			body: `{ switch (value._tag) {
				case "A": work(); break
			} }`,
		},
		{
			name: "switch default must be final",
			body: `{ switch (value._tag) {
				default: return fallback
				case "A": return result
			} }`,
		},
		{
			name: "conditional true arm cannot contain dispatch",
			body: `value._tag === "A" ? (value._tag === "B" ? first : second) : fallback`,
		},
		{
			name: "tag comparison must use a string literal",
			body: `value._tag === tag ? result : fallback`,
		},
		{
			name: "discriminant must access _tag",
			body: `value.kind === "A" ? result : fallback`,
		},
		{
			name: "branch cannot contain extra statements",
			body: `{ if (value._tag === "A") { work(); return result }
				return fallback }`,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			_, body := parseTaggedDispatchTestBody(t, tt.body)
			if dispatch := parseTaggedDispatchSyntax(body); dispatch != nil {
				t.Fatalf("expected parse rejection, got %+v", dispatch)
			}
		})
	}
}

func TestParseTaggedDispatchRootSymbolValidation(t *testing.T) {
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

		function narrowedIf(error: Wrapper) {
			if (error.reason._tag === "ReasonA") return 1
			else if (error.reason._tag === "ReasonB") return 2
			else return 0
		}

		function narrowedTernary(error: Wrapper) {
			return error.reason._tag === "ReasonA" ? 1
				: error.reason._tag === "ReasonB" ? 2
				: 0
		}

		function differentValue(error: Wrapper) {
			if (other.reason._tag === "ReasonA") return 1
			return 0
		}

		function alternateChain(error: Wrapper) {
			if (error.alternateReason._tag === "ReasonA") return 1
			return 0
		}
	`)
	defer done()

	tests := []struct {
		name string
		tags []string
		ok   bool
	}{
		{name: "narrowedIf", tags: []string{"ReasonA", "ReasonB"}, ok: true},
		{name: "narrowedTernary", tags: []string{"ReasonA", "ReasonB"}, ok: true},
		{name: "differentValue", ok: false},
		{name: "alternateChain", tags: []string{"ReasonA"}, ok: true},
	}
	for _, tt := range tests {
		function := findTaggedDispatchTestFunction(t, sf, tt.name)
		parameters := GetFunctionLikeParameters(function)
		body := GetFunctionLikeBody(function)
		if parameters == nil || len(parameters.Nodes) != 1 || body == nil {
			t.Fatalf("%s: expected one parameter and a function body", tt.name)
		}
		rootSymbol := tp.GetSymbolAtLocation(parameters.Nodes[0].Name())
		dispatch := tp.ParseTaggedDispatch(body, rootSymbol)
		if !tt.ok {
			if dispatch != nil {
				t.Fatalf("%s: expected parse rejection, got %+v", tt.name, dispatch)
			}
			continue
		}
		if dispatch == nil {
			t.Fatalf("%s: expected tagged dispatch", tt.name)
		}
		tags := make([]string, len(dispatch.Branches))
		for index, branch := range dispatch.Branches {
			tags[index] = branch.Tag
		}
		if !slices.Equal(tags, tt.tags) {
			t.Fatalf("%s: tags = %v, want %v", tt.name, tags, tt.tags)
		}
	}
}

func findTaggedDispatchTestFunction(t *testing.T, sf *ast.SourceFile, name string) *ast.Node {
	t.Helper()
	var found *ast.Node
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindFunctionDeclaration && node.Name() != nil && node.Name().Text() == name {
			found = node
			return true
		}
		return node.ForEachChild(walk)
	}
	walk(sf.AsNode())
	if found == nil {
		t.Fatalf("function %s not found", name)
	}
	return found
}

func parseTaggedDispatchTestBody(t *testing.T, bodySource string) (*ast.SourceFile, *ast.Node) {
	t.Helper()
	sf := parseSource("const decode = () => " + bodySource)
	var body *ast.Node
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindArrowFunction {
			function := node.AsArrowFunction()
			if function != nil {
				body = function.Body
			}
			return true
		}
		return node.ForEachChild(walk)
	}
	walk(sf.AsNode())
	if body == nil {
		t.Fatal("failed to parse arrow function body")
	}
	return sf, body
}

func taggedDispatchNodeText(sf *ast.SourceFile, node *ast.Node) string {
	if sf == nil || node == nil {
		return ""
	}
	return strings.TrimSpace(scanner.GetSourceTextOfNodeFromSourceFile(sf, node, false))
}
