package effecttest

import (
	_ "embed"
	"fmt"
	"slices"
	"testing"
)

//go:embed testdata/inferredGeneratorContextualType.ts
var inferredGeneratorContextualTypeSource string

func TestIssue833InferredGeneratorNativeDiagnostics(t *testing.T) {
	t.Parallel()

	const positiveControl = `
interface Options {
  /** @stability experimental */
  experimental?: number;
  /** @stability unstable */
  unstable?: number;
}
declare function accept(options: Options): void;
export const contextual = function* () {
  accept({ experimental: 1, unstable: 2 });
};
`
	for _, enabled := range []bool{false, true} {
		t.Run(fmt.Sprintf("rules=%t", enabled), func(t *testing.T) {
			t.Parallel()
			severity := "off"
			if enabled {
				severity = "warning"
			}
			content := fmt.Sprintf(`// @filename: tsconfig.json
{
  "compilerOptions": {
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "strict": true,
    "noEmit": true,
    "skipLibCheck": true,
    "plugins": [{
      "name": "@effect/language-service",
      "diagnosticSeverity": {
        "experimentalApiUsage": %q,
        "unstableApiUsage": %q,
        "lazyEffect": "off",
        "unnecessaryEffectGen": "off"
      }
    }]
  }
}
// @filename: package.json
{ "type": "module" }
// @filename: test.ts
`, severity, severity) + inferredGeneratorContextualTypeSource + positiveControl
			got := collectDiagnosticStringsFromContent(t, content)
			slices.Sort(got)
			var want []string
			if enabled {
				want = []string{
					"TS377135: `experimental` is an experimental API. effect(experimentalApiUsage)",
					"TS377136: `unstable` is an unstable API. Breaking changes may happen between versions. effect(unstableApiUsage)",
				}
			}
			if !slices.Equal(got, want) {
				t.Fatalf("diagnostics = %v, want %v", got, want)
			}
		})
	}
}
