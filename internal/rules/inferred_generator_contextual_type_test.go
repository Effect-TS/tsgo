package rules

import (
	"fmt"
	"slices"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

func TestInferredGeneratorStabilityPreservesNativeErrors(t *testing.T) {
	t.Parallel()

	const source = `interface Options {
  /** @stability experimental */
  experimental?: number;
  /** @stability unstable */
  unstable?: number;
}
declare function accept(options: Options): void;
declare function constrained<T extends Options>(options: T): T;
constrained({ experimental: 1, unstable: 2 });
const inferred = function* () {
  accept({ experimental: 3, unstable: 4 });
  constrained({ experimental: 5, unstable: 6 });
  yield* 1;
  const invalid = { ...1 };
};
const annotated = function* (): Generator<never, Options, unknown> {
  return constrained({ experimental: 7, unstable: 8 });
};
`
	for _, prime := range []bool{false, true} {
		for _, experimentalFirst := range []bool{false, true} {
			t.Run(fmt.Sprintf("prime=%t/experimentalFirst=%t", prime, experimentalFirst), func(t *testing.T) {
				t.Parallel()
				ctx, sf, done := compileApiStabilitySourcePrimed(t, source, prime)
				defer done()
				tests := []struct {
					rule rule.Rule
					uses []string
				}{
					{ExperimentalApiUsage, []string{"experimental: 1", "experimental: 3", "experimental: 7"}},
					{UnstableApiUsage, []string{"unstable: 2", "unstable: 4", "unstable: 8"}},
				}
				if !experimentalFirst {
					slices.Reverse(tests)
				}
				for _, test := range tests {
					diagnostics := test.rule.Run(ctx)
					if len(diagnostics) != len(test.uses) {
						t.Fatalf("%s: diagnostics = %v, want the outside, normal-context, and annotated-generator warnings", test.rule.Name, diagnostics)
					}
					for i, diagnostic := range diagnostics {
						name := strings.Split(test.uses[i], ":")[0]
						if diagnostic.Pos() != strings.Index(source, test.uses[i]) || source[diagnostic.Pos():diagnostic.Pos()+diagnostic.Len()] != name {
							t.Errorf("%s: diagnostic %d has range %d:%d, want %q", test.rule.Name, i, diagnostic.Pos(), diagnostic.Len(), test.uses[i])
						}
						if !strings.Contains(diagnostic.String(), "effect("+test.rule.Name+")") {
							t.Errorf("unexpected stability diagnostic: %s", diagnostic.String())
						}
					}
				}
				diagnostics := compiler.SortAndDeduplicateDiagnostics(ctx.Checker.GetDiagnostics(ctx.Context, sf))
				var got []string
				for _, diagnostic := range diagnostics {
					if diagnostic.Category() == tsdiag.CategoryError {
						got = append(got, fmt.Sprintf("%d:%d:%d", diagnostic.Code(), diagnostic.Pos(), diagnostic.Len()))
					}
				}
				slices.Sort(got)
				want := []string{
					fmt.Sprintf("2488:%d:1", strings.Index(source, "yield* 1")+len("yield* ")),
					fmt.Sprintf("2698:%d:4", strings.Index(source, "...1")),
				}
				if !slices.Equal(got, want) {
					t.Fatalf("native diagnostic ranges = %v, want %v", got, want)
				}
			})
		}
	}
}
