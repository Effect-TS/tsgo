package rules

import (
	"fmt"
	"slices"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/rule"

	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/parser"
)

func TestStabilityOfDeclaration(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name   string
		source string
		want   string
	}{
		{"unstable variable", "/** @stability unstable */\nexport const api = 1", "unstable"},
		{"experimental variable", "/** @stability experimental */\nexport const api = 1", "experimental"},
		{"stable variable", "/** @stability stable */\nexport const api = 1", "stable"},
		{"other tag", "/** @deprecated */\nexport const api = 1", ""},
		{"plain variable", "export const api = 1", ""},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			sf := parser.ParseSourceFile(ast.SourceFileParseOptions{FileName: "/test.ts"}, test.source, core.ScriptKindTS)
			statement := sf.Statements.Nodes[0]
			declaration := statement.AsVariableStatement().DeclarationList.AsVariableDeclarationList().Declarations.Nodes[0]
			if got := typeparser.StabilityTagOfDeclaration(declaration); got != test.want {
				t.Errorf("StabilityTagOfDeclaration = %q, want %q", got, test.want)
			}
		})
	}
}

func TestStabilityApiModuleName(t *testing.T) {
	t.Parallel()
	for _, test := range []struct{ file, want string }{
		{"/pkg/src/http/HttpClient.ts", "effect/http/HttpClient"},
		{"/pkg/dist/http/HttpClient.d.ts", "effect/http/HttpClient"},
		{"/pkg/dist/dts/http/HttpClient.d.mts", "effect/http/HttpClient"},
		{"/pkg/dist/cjs/http/HttpClient.d.cts", "effect/http/HttpClient"},
		{"/pkg/dist/http/index.d.ts", "effect/http"},
		{"/pkg/index.ts", "effect"},
		{"/pkg/types/client.d.ts", "effect/types/client"},
		{"/pkg-other/client.ts", ""},
	} {
		t.Run(test.file, func(t *testing.T) {
			t.Parallel()
			if got := stabilityApiModuleName("effect", "/pkg", test.file); got != test.want {
				t.Fatalf("got %q, want %q", got, test.want)
			}
		})
	}
}

func TestStabilityApiUsageContextualRuleOrder(t *testing.T) {
	t.Parallel()
	repeated := func(call string) string {
		return `interface Options {
  /** @stability experimental */
  first: number
  stable?: number
  /** @stability unstable */
  second: number
  nested?: Options
}

declare function accept(options: Options): void
declare function generic<T extends Options>(options: T): void
` + call + `({ first: 1, stable: 2, second: 3, nested: { first: 4, second: 5 } })
const unrelated = { first: 8, second: 9 }
accept({ first: 6, second: 7 })
`
	}
	repeatedExperimental := []string{"first: 1", "first: 4", "first: 6"}
	repeatedUnstable := []string{"second: 3", "second: 5", "second: 7"}
	discriminants := `interface A {
  kind: "a"
  /** @stability experimental */
  x: number
  y?: number
}
interface B {
  kind: "b"
  x: number
  /** @stability unstable */
  y?: number
}
declare function pick<T extends A | B>(value: T): T
pick({ kind: "a", x: pick({ kind: "b", x: 1, y: 2 }).x, y: 3 })
pick({ kind: "b", y: pick({ kind: "a", x: 4 }).x, x: 5 })
`
	tests := []struct {
		name         string
		source       string
		prime        bool
		experimental []string
		unstable     []string
	}{
		{"repeated/accept", repeated("accept"), true, repeatedExperimental, repeatedUnstable},
		{"repeated/generic", repeated("generic"), true, repeatedExperimental, repeatedUnstable},
		{"discriminants/prime=true", discriminants, true, []string{"x: pick", "x: 4"}, []string{"y: 2", "y: pick"}},
		{"discriminants/prime=false", discriminants, false, []string{"x: pick", "x: 4"}, []string{"y: 2", "y: pick"}},
	}
	for _, test := range tests {
		rules := []struct {
			rule rule.Rule
			uses []string
		}{
			{ExperimentalApiUsage, test.experimental},
			{UnstableApiUsage, test.unstable},
		}
		for _, order := range [][2]int{{0, 1}, {1, 0}} {
			t.Run(test.name+"/"+rules[order[0]].rule.Name+"-first", func(t *testing.T) {
				t.Parallel()
				ctx, _, done := compileApiStabilitySourcePrimed(t, test.source, test.prime)
				defer done()
				for _, index := range order {
					expected := rules[index]
					var got, want []string
					for _, diagnostic := range expected.rule.Run(ctx) {
						got = append(got, fmt.Sprintf("%d:%s", diagnostic.Pos(), test.source[diagnostic.Pos():diagnostic.Pos()+diagnostic.Len()]))
					}
					for _, use := range expected.uses {
						name, _, _ := strings.Cut(use, ":")
						want = append(want, fmt.Sprintf("%d:%s", strings.Index(test.source, use), name))
					}
					if !slices.Equal(got, want) {
						t.Errorf("%s: got %v, want %v", expected.rule.Name, got, want)
					}
				}
			})
		}
	}
}

func BenchmarkStabilityApiUsageContextualProperties(b *testing.B) {
	for _, properties := range []int{1, 8, 32} {
		b.Run(fmt.Sprintf("properties=%d", properties), func(b *testing.B) {
			var source strings.Builder
			source.WriteString(`interface Options {
  /** @stability experimental */
  first: number
  [key: string]: number
}
declare function accept<T extends Options>(options: T): T
`)
			for range 32 {
				source.WriteString("accept({ first: 0")
				for property := 1; property < properties; property++ {
					fmt.Fprintf(&source, ", property%d: %d", property, property)
				}
				source.WriteString(" })\n")
			}
			ctx, _, done := compileApiStabilitySource(b, source.String())
			defer done()
			if diagnostics := ExperimentalApiUsage.Run(ctx); len(diagnostics) != 32 {
				b.Fatalf("got %d diagnostics, want 32", len(diagnostics))
			}
			b.ReportAllocs()
			b.ResetTimer()
			for b.Loop() {
				if diagnostics := ExperimentalApiUsage.Run(ctx); len(diagnostics) != 32 {
					b.Fatalf("got %d diagnostics, want 32", len(diagnostics))
				}
			}
		})
	}
}
