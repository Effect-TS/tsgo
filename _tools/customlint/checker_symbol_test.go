package customlint

import (
	"fmt"
	"testing"

	"golang.org/x/tools/go/analysis/analysistest"
)

func TestCheckerLocations(t *testing.T) {
	analysistest.Run(t, analysistest.TestData(), checkerLocationAnalyzer,
		"github.com/effect-ts/tsgo/internal/...",
		"github.com/effect-ts/tsgo/public",
		"github.com/other/internal/consumer",
		"github.com/effect-ts/tsgo/internalish")
}

func TestForwardingSelector(t *testing.T) {
	cases := []struct {
		name string
		body string
	}{
		{"forwarding", "return parser.checker.%[1]s(input%[2]s)"},
		{"earlier call", `parser.checker.%[1]s(input%[2]s) // want "use TypeParser.%[1]s"
return parser.checker.%[1]s(input%[2]s)`},
		{"earlier return", `if input != nil { return parser.checker.%[1]s(input%[2]s) } // want "use TypeParser.%[1]s"
return parser.checker.%[1]s(input%[2]s)`},
		{"nested call", `return parser.checker.%[1]s(parser.checker.%[1]s(input%[2]s)%[2]s) // want "use TypeParser.%[1]s" "use TypeParser.%[1]s"`},
		{"different argument", `return parser.checker.%[1]s(nil%[2]s) // want "use TypeParser.%[1]s"`},
		{"different field", `return parser.other.%[1]s(input%[2]s) // want "use TypeParser.%[1]s"`},
		{"different receiver", `other := parser
return other.checker.%[1]s(input%[2]s) // want "use TypeParser.%[1]s"`},
		{"checker alias", `c := parser.checker
return c.%[1]s(input%[2]s) // want "use TypeParser.%[1]s"`},
		{"parameter alias", `otherInput := input
return parser.checker.%[1]s(otherInput%[2]s) // want "use TypeParser.%[1]s"`},
		{"shadowed parameter", `if input := input; input != nil { parser.checker.%[1]s(input%[2]s) } // want "use TypeParser.%[1]s"
return parser.checker.%[1]s(input%[2]s)`},
		{"method value", `lookup := parser.checker.%[1]s // want "use TypeParser.%[1]s"
return lookup(input%[2]s)`},
		{"closure", `return func() *checker.Node { return parser.checker.%[1]s(input%[2]s) }() // want "use TypeParser.%[1]s"`},
		{"earlier closure", `func() { parser.checker.%[1]s(input%[2]s) }() // want "use TypeParser.%[1]s"
return parser.checker.%[1]s(input%[2]s)`},
		{"shadowed receiver", `if parser := parser; input != nil { parser.checker.%[1]s(input%[2]s) } // want "use TypeParser.%[1]s"
return parser.checker.%[1]s(input%[2]s)`},
	}
	for methodName, forwardingMethod := range checkerLocationMethods {
		extraArguments := ""
		if methodName == "GetContextualType" {
			extraArguments = ", flags"
		}
		for _, receiver := range []string{"*TypeParser", "TypeParser"} {
			for _, tc := range cases {
				t.Run(methodName+"/"+receiver+"/"+tc.name, func(t *testing.T) {
					runForwardingFixture(t, receiver, forwardingMethod, fmt.Sprintf(tc.body, methodName, extraArguments))
				})
			}
		}
	}
}

func TestForwardingMethodMapping(t *testing.T) {
	for _, tc := range []struct {
		method string
		body   string
	}{
		{"GetSymbolAtLocation", `return parser.checker.GetTypeAtLocation(input) // want "use TypeParser.GetTypeAtLocation"`},
		{"getTypeAtLocationUncached", `return parser.checker.GetSymbolAtLocation(input) // want "use TypeParser.GetSymbolAtLocation"`},
		{"GetTypeAtLocation", `return parser.checker.GetTypeAtLocation(input) // want "use TypeParser.GetTypeAtLocation"`},
	} {
		t.Run(tc.method, func(t *testing.T) {
			runForwardingFixture(t, "TypeParser", tc.method, tc.body)
		})
	}
}

func runForwardingFixture(t *testing.T, receiver, method, body string, extraParameters ...string) {
	t.Helper()
	parameters := "input *checker.Node"
	if method == "GetContextualType" {
		parameters += ", flags checker.ContextFlags"
	}
	for _, parameter := range extraParameters {
		parameters += ", " + parameter
	}
	dir, cleanup, err := analysistest.WriteFiles(map[string]string{
		"github.com/microsoft/TypeScript/tsc/internal/checker/checker.go": `package checker
type Node struct{}
type Checker struct{}
type ContextFlags int
func (*Checker) GetContextualType(n *Node, flags ContextFlags) *Node { return n }
func (*Checker) GetSymbolAtLocation(n *Node) *Node { return n }
func (*Checker) GetTypeAtLocation(n *Node) *Node { return n }`,
		"github.com/microsoft/TypeScript/tsc/shim/checker/shim.go": `package checker
import provider "github.com/microsoft/TypeScript/tsc/internal/checker"
type Checker = provider.Checker
type Node = provider.Node
type ContextFlags = provider.ContextFlags`,
		"github.com/effect-ts/tsgo/internal/typeparser/parser.go": `package typeparser
import "github.com/microsoft/TypeScript/tsc/shim/checker"
type TypeParser struct { checker, other *checker.Checker }
func (parser ` + receiver + `) ` + method + `(` + parameters + `) *checker.Node {
` + body + `
}`,
	})
	if err != nil {
		t.Fatal(err)
	}
	defer cleanup()
	analysistest.Run(t, dir, checkerLocationAnalyzer, typeParserPackage)
}

func TestContextualTypeForwardingFlags(t *testing.T) {
	cases := []struct {
		name            string
		body            string
		extraParameters []string
	}{
		{"different flags", `return parser.checker.GetContextualType(input, 0) // want "use TypeParser.GetContextualType"`, nil},
		{"aliased flags", `otherFlags := flags
return parser.checker.GetContextualType(input, otherFlags) // want "use TypeParser.GetContextualType"`, nil},
		{"shadowed flags", `if flags := flags; input != nil { return parser.checker.GetContextualType(input, flags) } // want "use TypeParser.GetContextualType"
return parser.checker.GetContextualType(input, flags)`, nil},
		{"extra parameter", `return parser.checker.GetContextualType(input, flags) // want "use TypeParser.GetContextualType"`, []string{"unused int"}},
	}
	for _, receiver := range []string{"*TypeParser", "TypeParser"} {
		for _, tc := range cases {
			t.Run(receiver+"/"+tc.name, func(t *testing.T) {
				runForwardingFixture(t, receiver, "GetContextualType", tc.body, tc.extraParameters...)
			})
		}
	}
}
