package typeparser

import "testing"

func TestApiStabilityInheritanceMinimumAndCacheIsolation(t *testing.T) {
	t.Parallel()
	for _, warm := range []bool{false, true} {
		name := "cold"
		if warm {
			name = "warm"
		}
		t.Run(name, func(t *testing.T) {
			t.Parallel()
			c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
				"/.src/test.ts": `/** @stability unstable */
interface Reportable {
/** @stability experimental */ test?: string }
declare global { interface Error extends Reportable {} }
export function augmented(e: Error) {}
export interface Expanded extends Error {}
export interface Ordinary extends Reportable {}
export function direct(e: Reportable) {}
`,
			})
			defer done()
			if warm {
				primeApiStabilityChecker(t, c)
			}
			sf := files["/.src/test.ts"]
			// Repeat with opposite root orders so neither shared surfaces nor an
			// analysis-local result can erase a directly exposed dependency.
			for _, order := range [][]string{
				{"augmented", "Expanded", "direct", "Ordinary"},
				{"Ordinary", "direct", "Expanded", "augmented"},
			} {
				for _, name := range order {
					symbol := apiStabilityTestExport(t, c, sf, name)
					if warm && name == "Expanded" {
						c.GetPropertiesOfType(c.GetDeclaredTypeOfSymbol(symbol))
					}
					used := tp.ApiStabilityUsedBySymbol(symbol)
					want := ApiStabilityStable
					if name == "direct" {
						want = ApiStabilityUnstable
					}
					if used.Minimum != want || (warm && used.Incomplete) {
						t.Errorf("%s minimum=%v incomplete=%v deps=%v, want %v", name, used.Minimum, used.Incomplete, apiStabilityDependencyNames(used), want)
					}
					if name == "Expanded" {
						// Also inspect its represented type after the compiler has
						// flattened the inherited member table.
						typ := c.GetDeclaredTypeOfSymbol(symbol)
						c.GetPropertiesOfType(typ)
						used = tp.ApiStabilityUsedByType(typ)
						if used.Incomplete || used.Minimum != ApiStabilityStable {
							t.Errorf("expanded type minimum=%v incomplete=%v deps=%v", used.Minimum, used.Incomplete, apiStabilityDependencyNames(used))
						}
					}
				}
			}
		})
	}
}
