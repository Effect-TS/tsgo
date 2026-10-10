package rules

import (
	"fmt"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

func TestStabilityApiUsageIndependentReporting(t *testing.T) {
	t.Parallel()
	for _, reverse := range []bool{false, true} {
		t.Run(fmt.Sprintf("reverse=%t", reverse), func(t *testing.T) {
			t.Parallel()
			ctx, done := compileApiStabilityFiles(t, map[string]string{
				"package.json": `{"name":"example"}`,
				"api.ts": `export interface First {
  /** @stability experimental */
  key?: number
}
export interface Second {
  /** @stability experimental */
  key?: number
}
export interface Third {
  /** @stability unstable */
  key?: number
}`,
				"test.ts": `import type { First, Second, Third } from "./api.js"
const value: First | Second | Third = { key: 1 }`,
			})
			defer done()
			rules := []rule.Rule{ExperimentalApiUsage, UnstableApiUsage}
			if reverse {
				rules[0], rules[1] = rules[1], rules[0]
			}
			// The first contextual declaration may be allowed while a later
			// declaration of the same stability still needs a diagnostic.
			for _, severity := range []etscore.Severity{etscore.SeverityWarning, etscore.SeverityError} {
				for _, allow := range []bool{true, false, true} {
					options := &etscore.ResolvedEffectPluginOptions{}
					if allow {
						options.AllowedExperimentalApis = []string{"example/api#First"}
						options.AllowedUnstableApis = []string{"example/api#Third"}
					}
					// Separate contexts and parsers, as in the diagnostic runner.
					for _, r := range rules {
						tp := typeparser.NewTypeParser(ctx.Program, ctx.Checker)
						invocation := rule.NewContext(ctx.Context, ctx.Program, ctx.Checker, tp, ctx.SourceFile, options, severity)
						diagnostics := r.Run(invocation)
						want := 1
						if allow && r.Name == UnstableApiUsage.Name {
							want = 0
						}
						if len(diagnostics) != want {
							t.Fatalf("%s allow=%t: got %d diagnostics, want %d: %v", r.Name, allow, len(diagnostics), want, diagnostics)
						}
						if want == 0 {
							continue
						}
						diagnostic := diagnostics[0]
						if allow && !strings.Contains(diagnostic.String(), "example/api#Second") {
							t.Fatalf("did not report the non-allowed declaration: %s", diagnostic.String())
						}
						category := tsdiag.CategoryWarning
						if severity == etscore.SeverityError {
							category = tsdiag.CategoryError
						}
						if diagnostic.Category() != category {
							t.Fatalf("got category %v, want %v", diagnostic.Category(), category)
						}
					}
				}
			}
		})
	}
}

func stabilityAnalysisSource(properties int) string {
	var source strings.Builder
	source.WriteString(`interface Options {
  /** @stability experimental */
  first: number
  /** @stability unstable */
  second: number
  [key: string]: number
}
declare function accept<T extends Options>(options: T): T
`)
	for range 32 {
		source.WriteString("accept({ first: 0, second: 1")
		for property := 2; property < properties; property++ {
			fmt.Fprintf(&source, ", property%d: %d", property, property)
		}
		source.WriteString(" })\n")
	}
	return source.String()
}

// Each iteration starts with empty Effect analysis caches. The compiler has
// already checked the source. This isolates rule work; it is not an end-to-end
// typechecking benchmark, nor a benchmark of repeatedly reading cached results.
func BenchmarkStabilityApiUsageColdAnalysis(b *testing.B) {
	for _, properties := range []int{2, 8, 32} {
		for _, both := range []bool{false, true} {
			b.Run(fmt.Sprintf("properties=%d/both=%t", properties, both), func(b *testing.B) {
				ctx, _, done := compileApiStabilitySource(b, stabilityAnalysisSource(properties))
				defer done()
				b.ReportAllocs()
				b.ResetTimer()
				for b.Loop() {
					ctx.Checker.EffectLinks = nil
					ctx.TypeParser = typeparser.NewTypeParser(ctx.Program, ctx.Checker)
					if diagnostics := ExperimentalApiUsage.Run(ctx); len(diagnostics) != 32 {
						b.Fatalf("got %d diagnostics, want 32", len(diagnostics))
					}
					if both {
						if diagnostics := UnstableApiUsage.Run(ctx); len(diagnostics) != 32 {
							b.Fatalf("got %d diagnostics, want 32", len(diagnostics))
						}
					}
				}
			})
		}
	}
}
