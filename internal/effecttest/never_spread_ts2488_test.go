package effecttest

import (
	"fmt"
	"slices"
	"testing"
)

// Issue #916: promiseInEffectSuccess and other rules queried the type of a
// spread element of a never-typed value, a node the checker only checks
// through its enclosing array literal or call, leaking TS2488 into the
// program's diagnostics. Stock checking accepts the array spread and rejects
// the call spread, and the plugin must report exactly the same.
func TestIssue916NeverSpreadMatchesStockDiagnostics(t *testing.T) {
	t.Parallel()

	const source = `declare function g(...values: Array<number>): void

export function f() {
  const rows = [1] as never;
  g(...rows);
  return [...rows];
}
`
	for name, plugin := range map[string]string{
		"no plugin":       ``,
		"diagnostics off": `{ "name": "@effect/language-service", "diagnostics": false }`,
		"rule off":        `{ "name": "@effect/language-service", "diagnosticSeverity": { "promiseInEffectSuccess": "off" } }`,
		"rule error":      `{ "name": "@effect/language-service", "diagnosticSeverity": { "promiseInEffectSuccess": "error" } }`,
	} {
		t.Run(name, func(t *testing.T) {
			t.Parallel()
			content := fmt.Sprintf(`// @filename: tsconfig.json
{ "compilerOptions": { "strict": true, "plugins": [%s] } }
// @filename: package.json
{ "type": "module" }
// @filename: test.ts
`, plugin) + source
			got := collectDiagnosticStringsFromContent(t, content)
			want := []string{"TS2488: Type 'never' must have a '[Symbol.iterator]()' method that returns an iterator."}
			if !slices.Equal(got, want) {
				t.Fatalf("diagnostics = %v, want %v", got, want)
			}
		})
	}
}
