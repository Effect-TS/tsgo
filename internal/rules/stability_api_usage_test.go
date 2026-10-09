package rules

import (
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/etscore"
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

func TestStabilityApiUsageContextualProperties(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name   string
		source string
		want   string
	}{
		{"argument", "x({ test: 42 })", "test"},
		{"shorthand", "const test = 42; x({ test })", "test"},
		{"quoted key", `x({ "test": 42 })`, `"test"`},
		{"computed literal key", `x({ ["test"]: 42 })`, `["test"]`},
		{"typed variable", "const value: Reportable = { test: 42 }", "test"},
		{"assignment", "let value: Reportable; value = { test: 42 }", "test"},
		{"satisfies", "const value = { test: 42 } satisfies Reportable", "test"},
		{"return", "function make(): Reportable { return { test: 42 } }", "test"},
		{"nested", "const value: { inner: Reportable } = { inner: { test: 42 } }", "test"},
		{"array element", "const value: Reportable[] = [{ test: 42 }]", "test"},
		{"generic constraint", "declare function generic<T extends Reportable>(arg: T): void; generic({ test: 42 })", "test"},
		{"generic own declaration", "declare function generic<T>(arg: T): void; generic({\n/** @stability TAG */\ntest: 42 })", ""},
		{"inherited", "interface Child extends Reportable {} const value: Child = { test: 42 }", "test"},
		{"property access", "declare function make(): Reportable; make().test", "test"},
		{"value reference", "/** @stability TAG */\nconst value = 42; x({ stable: value })", "value"},
		{"stable property", "x({ stable: 42 })", ""},
		{"omitted property", "x({})", ""},
		{"untyped literal", "const value = { test: 42 }; x(value)", ""},
		{"untyped shorthand", "const test = 42; const value = { test }; x(value)", ""},
		{"spread", "const value = { test: 42 }; x({ ...value })", ""},
		{"different property", "const value: { test?: number } = { test: 42 }", ""},
		{"stable union branch", `type Choice = (Reportable & { kind: "experimental" }) | { kind: "stable"; test?: number }; const value: Choice = { kind: "stable", test: 42 }`, ""},
		{"experimental union branch", `type Choice = (Reportable & { kind: "experimental" }) | { kind: "stable"; test?: number }; const value: Choice = { kind: "experimental", test: 42 }`, "test"},
		{"ambiguous union", "type Choice = Reportable | { test?: string }; const value: Choice = { test: 42 }", "test"},
		{"union deduplication", `type Choice = (Reportable & { kind?: "a" }) | (Reportable & { kind?: "b" }); const value: Choice = { test: 42 }`, "test"},
		{"method", "/** @stability stable */\ninterface Methods {\n/** @stability TAG */\ntest(): number }; const value: Methods = { test() { return 42 } }", "test"},
	}
	for _, taggedRule := range []struct {
		tag  string
		rule rule.Rule
	}{
		{"experimental", ExperimentalApiUsage},
		{"unstable", UnstableApiUsage},
	} {
		for _, test := range tests {
			t.Run(taggedRule.tag+"/"+test.name, func(t *testing.T) {
				t.Parallel()
				source := strings.ReplaceAll(`/** @stability stable */
interface Reportable {
  /** @stability TAG */
  readonly test?: number
  stable?: number
}
/** @stability stable */
declare function x(arg: Reportable): void
`+test.source, "TAG", taggedRule.tag)
				ctx, _, done := compileApiStabilitySource(t, source)
				defer done()
				diagnostics := taggedRule.rule.Run(ctx)
				if test.want == "" {
					if len(diagnostics) != 0 {
						t.Fatalf("got diagnostics %v, want none", diagnostics)
					}
					return
				}
				if len(diagnostics) != 1 {
					t.Fatalf("got %d diagnostics, want 1: %v", len(diagnostics), diagnostics)
				}
				diagnostic := diagnostics[0]
				if got := source[diagnostic.Pos() : diagnostic.Pos()+diagnostic.Len()]; got != test.want {
					t.Errorf("diagnostic highlights %q, want %q", got, test.want)
				}
				if !strings.Contains(diagnostic.String(), "effect("+taggedRule.rule.Name+")") {
					t.Errorf("unexpected diagnostic: %s", diagnostic.String())
				}
			})
		}
	}
}

func TestStabilityApiUsageContextualPropertiesAllowlist(t *testing.T) {
	t.Parallel()
	for _, allowed := range []string{"", "example", "example/api", "example/api#Reportable", "example/api#other"} {
		t.Run(allowed, func(t *testing.T) {
			t.Parallel()
			ctx, done := compileApiStabilityFiles(t, map[string]string{
				"package.json": `{"name":"example"}`,
				"api.ts": `/** @stability stable */
export interface Reportable {
  /** @stability experimental */
  readonly test?: number
}`,
				"test.ts": `import type { Reportable } from "./api.js"
declare function x(arg: Reportable): void
x({ test: 42 })`,
			})
			defer done()
			ctx.Options = &etscore.ResolvedEffectPluginOptions{AllowedExperimentalApis: []string{allowed}}
			diagnostics := ExperimentalApiUsage.Run(ctx)
			want := 0
			if allowed == "" || allowed == "example/api#other" {
				want = 1
			}
			if len(diagnostics) != want {
				t.Fatalf("got %d diagnostics, want %d: %v", len(diagnostics), want, diagnostics)
			}
			if want == 1 && !strings.Contains(diagnostics[0].String(), "`example/api#Reportable`") {
				t.Errorf("unexpected API name: %s", diagnostics[0].String())
			}
		})
	}
}
