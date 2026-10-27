package effecttest_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/effecttest"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/fourslash"
	"github.com/microsoft/TypeScript/tsc/shim/ls/lsconv"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
)

func TestManualMetricTapToTrackFixesCompile(t *testing.T) {
	t.Parallel()
	version := bundledeffect.EffectV4
	const prefix = `// @filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "skipLibCheck": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [{
      "name": "@effect/language-service",
      "diagnosticSeverity": { "*": "off", "manualMetricTapToTrack": "suggestion" }
    }]
  }
}
// @filename: /repro.ts
`
	cases := []struct {
		name     string
		source   string
		expected string
		matches  int
		fixes    int
	}{
		{name: "manualMetricTapToTrack.ts", matches: 30, fixes: 30},
		{name: "manualMetricTapToTrack_preview.ts", matches: 1, fixes: 1},
		{name: "manualMetricTapToTrack_noTarget.ts", matches: 3, fixes: 0},
		{name: "manualMetricTapToTrack_bindings.ts", matches: 7, fixes: 6},
		{name: "manualMetricTapToTrack_localAliases.ts", matches: 3, fixes: 2},
		{
			name: "dataFirstCallback",
			source: `import { Effect, Metric } from "effect"
const responseSize = Metric.gauge("response_size")
declare const source: Effect.Effect<number>
export const value = Effect.tap(source, (size) => Metric.update(responseSize, size))
`,
			expected: `import { Effect, Metric } from "effect"
const responseSize = Metric.gauge("response_size")
declare const source: Effect.Effect<number>
export const value = Effect.trackSuccesses(source, responseSize)
`,
			matches: 1, fixes: 1,
		},
		{
			name: "dataFirstDirect",
			source: `import { Effect, Metric } from "effect"
const successes = Metric.counter("successes")
declare const source: Effect.Effect<number>
export const value = Effect.tap(source, Metric.update(successes, 1))
`,
			expected: `import { Effect, Metric } from "effect"
const successes = Metric.counter("successes")
declare const source: Effect.Effect<number>
export const value = Effect.trackSuccesses(source, Metric.withConstantInput(successes, 1))
`,
			matches: 1, fixes: 1,
		},
		{
			name: "directNamespaceMetric",
			source: `import { Effect } from "effect"
import * as M from "effect/Metric"
const successes = M.counter("successes")
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.tap(M.update(successes, 1)))
`,
			expected: `import { Effect } from "effect"
import * as M from "effect/Metric"
const successes = M.counter("successes")
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.trackSuccesses(M.withConstantInput(successes, 1)))
`,
			matches: 1, fixes: 1,
		},
		{
			name: "directUpdateAliasFallback",
			source: `import { Effect, Metric } from "effect"
const successes = Metric.counter("successes")
const updateAlias = Metric.update
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.tap(updateAlias(successes, 1)))
`,
			expected: `import { Effect, Metric } from "effect"
const successes = Metric.counter("successes")
const updateAlias = Metric.update
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.trackSuccesses(Metric.withConstantInput(successes, 1)))
`,
			matches: 1, fixes: 1,
		},
		{
			name: "tapAliasFallback",
			source: `import * as Fx from "effect/Effect"
import { Metric } from "effect"
const responseSize = Metric.gauge("response_size")
const tapAlias = Fx.tap
declare const source: Fx.Effect<number>
export const value = source.pipe(tapAlias((size) => Metric.update(responseSize, size)))
`,
			expected: `import * as Fx from "effect/Effect"
import { Metric } from "effect"
const responseSize = Metric.gauge("response_size")
const tapAlias = Fx.tap
declare const source: Fx.Effect<number>
export const value = source.pipe(Fx.trackSuccesses(responseSize))
`,
			matches: 1, fixes: 1,
		},
		{
			name: "destructuredMapper",
			source: `import { Effect, Metric } from "effect"
const userCount = Metric.gauge("user_count")
declare const source: Effect.Effect<ReadonlyArray<string>>
export const value = source.pipe(Effect.tap(({ length }) => Metric.update(userCount, length)))
`,
			expected: `import { Effect, Metric } from "effect"
const userCount = Metric.gauge("user_count")
declare const source: Effect.Effect<ReadonlyArray<string>>
export const value = source.pipe(Effect.trackSuccesses(userCount, ({ length }) => length))
`,
			matches: 1, fixes: 1,
		},
		{
			name: "restParameterMapper",
			source: `import { Effect, Metric } from "effect"
const calls = Metric.gauge("calls")
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.tap((...args: Array<number>) => Metric.update(calls, args.length)))
`,
			expected: `import { Effect, Metric } from "effect"
const calls = Metric.gauge("calls")
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.trackSuccesses(calls, (...args: Array<number>) => args.length))
`,
			matches: 1, fixes: 1,
		},
		{
			name: "defaultParameterMapper",
			source: `import { Effect, Metric } from "effect"
const sizes = Metric.gauge("sizes")
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.tap((size = 0) => Metric.update(sizes, size)))
`,
			expected: `import { Effect, Metric } from "effect"
const sizes = Metric.gauge("sizes")
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.trackSuccesses(sizes, (size = 0) => size))
`,
			matches: 1, fixes: 1,
		},
		{
			name: "effectFnPipeable",
			source: `import { Effect, Metric } from "effect"
const responseSize = Metric.gauge("response_size")
export const handler = Effect.fn("handler")(
  function*(id: string) {
    return id.length
  },
  Effect.tap((size) => Metric.update(responseSize, size))
)
`,
			expected: `import { Effect, Metric } from "effect"
const responseSize = Metric.gauge("response_size")
export const handler = Effect.fn("handler")(
  function*(id: string) {
    return id.length
  },
  Effect.trackSuccesses(responseSize)
)
`,
			matches: 1, fixes: 1,
		},
		{
			name: "dataLastApplied",
			source: `import { Effect, Metric } from "effect"
const requestErrors = Metric.counter("request_errors")
declare const source: Effect.Effect<number, Error>
export const value = Effect.tapError(() => Metric.update(requestErrors, 1))(source)
`,
			expected: `import { Effect, Metric } from "effect"
const requestErrors = Metric.counter("request_errors")
declare const source: Effect.Effect<number, Error>
export const value = Effect.trackErrors(requestErrors, () => 1)(source)
`,
			matches: 1, fixes: 1,
		},
		{
			// Only direct member imports reach the target APIs.
			name: "directMemberImports",
			source: `import type { Effect } from "effect"
import { tap, trackSuccesses } from "effect/Effect"
import { counter, update, withConstantInput } from "effect/Metric"
const successes = counter("successes")
declare const source: Effect.Effect<number>
export const value = source.pipe(tap(update(successes, 1)))
`,
			expected: `import type { Effect } from "effect"
import { tap, trackSuccesses } from "effect/Effect"
import { counter, update, withConstantInput } from "effect/Metric"
const successes = counter("successes")
declare const source: Effect.Effect<number>
export const value = source.pipe(trackSuccesses(withConstantInput(successes, 1)))
`,
			matches: 1, fixes: 1,
		},
		{
			// Effect.trackSuccesses is reachable but Metric.withConstantInput is not.
			name: "noMetricTarget",
			source: `import { Effect } from "effect"
import { counter, update } from "effect/Metric"
const successes = counter("successes")
declare const source: Effect.Effect<number>
export const value = source.pipe(Effect.tap(update(successes, 1)))
`,
			matches: 1, fixes: 0,
		},
	}
	code := tsdiag.Effect_0_expresses_recording_a_metric_on_every_1_directly_effect_manualMetricTapToTrack.Code()
	uri := lsconv.FileNameToDocumentURI(tspath.RootedFilePath("/repro.ts"))
	countMatches := func(f *fourslash.FourslashTest) int {
		count := 0
		for _, diagnostic := range f.GetQuickFixesForDiagnostics(t, uri) {
			if diagnostic.Diagnostic.Code != nil && diagnostic.Diagnostic.Code.Integer != nil && *diagnostic.Diagnostic.Code.Integer == code {
				count++
			}
		}
		return count
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			source := test.source
			if source == "" {
				fixture, err := os.ReadFile(filepath.Join(effecttest.TestDataPath(), "tests", string(version), test.name))
				if err != nil {
					t.Fatal(err)
				}
				// The fixtures raise the rule to a warning; keep it a suggestion so that
				// only real compile errors fail VerifyNonSuggestionDiagnostics.
				source = strings.Replace(string(fixture), "manualMetricTapToTrack:warning", "manualMetricTapToTrack:suggestion", 1)
			}
			source = "// @" + string(version) + "\n" + source
			f, done := fourslash.NewFourslash(t, nil, prefix+source)
			defer done()
			f.GoToFile(t, "/repro.ts")
			f.VerifyNonSuggestionDiagnostics(t, nil)
			matches, fixes := 0, 0
			for _, diagnostic := range f.GetQuickFixesForDiagnostics(t, uri) {
				if diagnostic.Diagnostic.Code == nil || diagnostic.Diagnostic.Code.Integer == nil || *diagnostic.Diagnostic.Code.Integer != code {
					continue
				}
				matches++
				for index, title := range diagnostic.FixTitles {
					if !strings.HasPrefix(title, "Replace with Effect.track") {
						continue
					}
					result := f.ApplyQuickFix(t, uri, diagnostic.Diagnostic, index)
					if len(result.Changes) != 1 {
						t.Fatalf("expected one changed file, got %d", len(result.Changes))
					}
					after := result.Changes[0].After
					if test.expected != "" && after != "// @"+string(version)+"\n"+test.expected {
						t.Fatalf("replacement = %q, want %q", after, "// @"+string(version)+"\n"+test.expected)
					}
					func() {
						converted, closeConverted := fourslash.NewFourslash(t, nil, prefix+after)
						defer closeConverted()
						converted.GoToFile(t, "/repro.ts")
						converted.VerifyNonSuggestionDiagnostics(t, nil)
						if remaining := countMatches(converted); remaining != test.matches-1 {
							t.Fatalf("expected %d diagnostics after replacement, got %d", test.matches-1, remaining)
						}
					}()
					fixes++
				}
			}
			if matches != test.matches || fixes != test.fixes {
				t.Fatalf("got %d diagnostics and %d compiled fixes, want %d and %d", matches, fixes, test.matches, test.fixes)
			}
		})
	}
}
