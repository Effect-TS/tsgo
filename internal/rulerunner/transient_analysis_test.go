package rulerunner_test

import (
	"context"
	"reflect"
	"testing"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
)

// Not parallel: CLI mode is process-global.
func TestCommandLineAnalysisCacheLifetime(t *testing.T) { //nolint:paralleltest
	var reference []string
	for _, cli := range []bool{false, true} {
		func() {
			if cli {
				defer etscore.EnterCommandLineMode()()
			}
			program, c, done := apiStabilityProgram(t, map[string]string{
				"api.ts": `/** @stability experimental */
export declare function make<T>(value: T): T`,
				"first.ts": `import { make } from "./api.js"
export const first = make({ value: 1 })`,
				"second.ts": `import { make } from "./api.js"
export const second = make({ value: 2 })`,
			}, false)
			defer done()
			program.Options().Effect.DiagnosticSeverity["experimentalApiUsage"] = etscore.SeverityWarning
			program.Options().Effect.DiagnosticSeverity["effectInVoidSuccess"] = etscore.SeverityWarning
			var diagnostics []string
			for _, path := range []string{"/.src/first.ts", "/.src/second.ts"} {
				sf := program.GetSourceFile(tspath.RootedFilePath(path))
				for _, diagnostic := range c.GetDiagnostics(context.Background(), sf) {
					diagnostics = append(diagnostics, diagnostic.String())
				}
				links := c.EffectLinks.(*typeparser.EffectLinks)
				if got := links.ExpectedAndRealTypes.Has(sf); got == cli {
					t.Errorf("CLI=%t: completed file cache retained=%t", cli, got)
				}
				if got := links.ApiStabilityUsages.Has(sf); got == cli {
					t.Errorf("CLI=%t: stability usage cache retained=%t", cli, got)
				}
			}
			if len(diagnostics) != 2 {
				t.Fatalf("CLI=%t: got %d diagnostics, want 2: %v", cli, len(diagnostics), diagnostics)
			}
			if cli {
				if !reflect.DeepEqual(diagnostics, reference) {
					t.Fatalf("CLI diagnostics differ: %v != %v", diagnostics, reference)
				}
			} else {
				reference = diagnostics
			}
		}()
	}
}
