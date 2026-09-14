package rules

import (
	"context"
	"fmt"
	"strings"
	"testing"
	"testing/fstest"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/bundled"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

var benchmarkTimeoutMatches []TimeoutCatchTagToTimeoutOrElseMatch

// benchmarkTimeoutFixture builds a checked source file outside the timed loop.
func benchmarkTimeoutFixture(b *testing.B, scenario string) (*typeparser.TypeParser, *checker.Checker, *ast.SourceFile) {
	b.Helper()
	var source strings.Builder
	source.WriteString(`import { Effect } from "effect"; declare const task: Effect.Effect<string>;
`)
	for i := range 128 {
		body := `Effect.map(x => x.length), Effect.map(x => x+1), Effect.map(x => String(x)), Effect.map(x => x.trim())`
		if scenario == "dense" || scenario == "sparse" && i == 127 {
			body = `Effect.timeout(1), Effect.catchTag("TimeoutError", () => Effect.succeed("fallback"))`
		}
		if scenario == "option" {
			body = `Effect.timeout(1), Effect.asSome, Effect.catchTag("TimeoutError", () => Effect.succeedNone)`
		}
		fmt.Fprintf(&source, "export const task%d = task.pipe(%s);\n", i, body)
	}
	files := map[string]any{"/.src/main.ts": &fstest.MapFile{Data: []byte(source.String())}}
	if err := bundledeffect.MountEffect(bundledeffect.EffectV4, files); err != nil {
		b.Fatal(err)
	}
	fs := bundled.WrapFS(vfstest.FromMap(files, true))
	options := &core.CompilerOptions{Target: core.ScriptTargetESNext, Module: core.ModuleKindNodeNext, ModuleResolution: core.ModuleResolutionKindNodeNext, Strict: core.TSTrue, SkipLibCheck: core.TSTrue}
	program := compiler.NewProgram(compiler.ProgramOptions{
		Config: &tsoptions.ParsedCommandLine{ParsedConfig: &core.ParsedOptions{CompilerOptions: options, FileNames: []string{"/.src/main.ts"}}},
		Host:   compiler.NewCompilerHost("/.src", fs, bundled.LibPath(), nil, nil), SingleThreaded: core.TSTrue,
	})
	if diags := program.GetSemanticDiagnostics(context.Background(), nil); len(diags) != 0 {
		b.Fatalf("fixture has %d diagnostics: %v", len(diags), diags[0])
	}
	c, done := program.GetTypeChecker(context.Background())
	b.Cleanup(done)
	return typeparser.NewTypeParser(program, c), c, program.GetSourceFile("/.src/main.ts")
}

// BenchmarkTimeoutRule measures warmed source-file analysis. Compilation and
// initial parsing/checker cache population are excluded from timing.
func BenchmarkTimeoutRule(b *testing.B) {
	for _, scenario := range []string{"no_match", "sparse", "dense", "option"} {
		b.Run(scenario, func(b *testing.B) {
			tp, c, sf := benchmarkTimeoutFixture(b, scenario)
			expected := 0
			switch scenario {
			case "sparse":
				expected = 1
			case "dense", "option":
				expected = 128
			}
			matches := AnalyzeTimeoutCatchTagToTimeoutOrElse(tp, c, sf)
			if len(matches) != expected {
				b.Fatalf("got %d matches, want %d", len(matches), expected)
			}
			b.ReportAllocs()
			b.ResetTimer()
			for b.Loop() {
				benchmarkTimeoutMatches = AnalyzeTimeoutCatchTagToTimeoutOrElse(tp, c, sf)
			}
		})
	}
}
