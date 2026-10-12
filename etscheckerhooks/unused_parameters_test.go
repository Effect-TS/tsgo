package etscheckerhooks_test

import (
	"fmt"
	"reflect"
	"testing"
	"testing/fstest"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/microsoft/TypeScript/tsc/shim/bundled"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/fourslash"
	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

func TestEffectDiagnosticsPreserveUnusedPredicateParameterLSP(t *testing.T) {
	t.Parallel()
	for _, enabled := range []bool{false, true} {
		t.Run(fmt.Sprintf("plugin enabled=%t", enabled), func(t *testing.T) {
			t.Parallel()
			plugins := "[]"
			if enabled {
				plugins = `[{"name":"@effect/language-service"}]`
			}
			content := fmt.Sprintf(`// @Filename: /tsconfig.json
{"compilerOptions":{"strict":true,"noUnusedLocals":true,"noUnusedParameters":true,"target":"ES2023","types":[],"plugins":%s},"files":["main.ts"]}
// @Filename: /main.ts
export function unusedPredicate([|value|]: unknown): value is number {
  return true;
}`, plugins)
			f, done := fourslash.NewFourslash(t, nil, content)
			defer done()
			f.GoToFile(t, "/main.ts")
			for range 3 {
				f.VerifyDiagnostics(t, []*lsproto.Diagnostic{{
					Code:    &lsproto.IntegerOrString{Integer: new(int32(6133))},
					Message: lsproto.StringOrMarkupContent{String: new("'value' is declared but its value is never read.")},
					Range:   f.Ranges()[0].LSRange,
					Tags:    &[]lsproto.DiagnosticTag{lsproto.DiagnosticTagUnnecessary},
				}})
			}
		})
	}
}

func TestEffectDiagnosticsPreserveUnusedPredicateParameter(t *testing.T) {
	t.Parallel()
	for _, enabled := range []bool{false, true} {
		name := "plugin disabled"
		if enabled {
			name = "plugin enabled"
		}
		t.Run(name, func(t *testing.T) {
			t.Parallel()
			program := unusedParameterProgram(enabled, true, `export function unusedPredicate(value: unknown): value is number {
  return true;
}`)
			sourceFile := program.GetSourceFile("/src/main.ts")
			for request := range 3 {
				diagnostics := program.GetSemanticDiagnostics(t.Context(), sourceFile)
				if len(diagnostics) != 1 {
					t.Fatalf("request %d: expected TS6133, got %v", request, diagnostics)
				}
				diagnostic := diagnostics[0]
				if diagnostic.Code() != 6133 || diagnostic.Pos() != 32 || diagnostic.End() != 37 || diagnostic.String() != "'value' is declared but its value is never read." {
					t.Fatalf("request %d: unexpected diagnostic: %v", request, diagnostic)
				}
				if suggestions := program.GetSuggestionDiagnostics(t.Context(), sourceFile); len(suggestions) != 0 {
					t.Fatalf("request %d: unexpected suggestions: %v", request, suggestions)
				}
			}
		})
	}
}

func TestEffectDiagnosticsRunWithoutUnusedChecking(t *testing.T) {
	t.Parallel()
	program := unusedParameterProgram(true, false, `export function unusedPredicate(value: unknown): value is number {
  return true;
}
export class LocalProblem extends Error {}`)
	sourceFile := program.GetSourceFile("/src/main.ts")
	for request := range 3 {
		var codes []int32
		for _, diagnostic := range program.GetSemanticDiagnostics(t.Context(), sourceFile) {
			codes = append(codes, diagnostic.Code())
		}
		if !reflect.DeepEqual(codes, []int32{377055}) {
			t.Fatalf("request %d: expected one extendsNativeError diagnostic, got %v", request, codes)
		}
	}
}

func unusedParameterProgram(pluginEnabled, unusedEnabled bool, source string) *compiler.Program {
	fs := bundled.WrapFS(vfstest.FromMap(map[string]any{
		"/src/main.ts": &fstest.MapFile{Data: []byte(source)},
	}, tspath.CaseSensitive))
	options := &core.CompilerOptions{
		Strict:              core.TSTrue,
		Target:              core.ScriptTargetES2023,
		NoUnusedLocals:      core.BoolToTristate(unusedEnabled),
		NoUnusedParameters:  core.BoolToTristate(unusedEnabled),
		SkipDefaultLibCheck: core.TSTrue,
	}
	if pluginEnabled {
		options.Effect = &etscore.EffectPluginOptions{
			Diagnostics: true,
			DiagnosticSeverity: map[string]etscore.Severity{
				"extendsNativeError": etscore.SeverityError,
			},
		}
	}
	return compiler.NewProgram(compiler.ProgramOptions{
		Config:         tsoptions.NewParsedCommandLine(options, []tspath.RootedFilePath{"/src/main.ts"}, nil, "/", tspath.CaseSensitive),
		Host:           compiler.NewCompilerHost(fs, bundled.LibPath(), nil, nil, nil),
		SingleThreaded: core.TSTrue,
	})
}
