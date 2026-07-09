package etscheckerhooks_test

import (
	"context"
	"strings"
	"testing"
	"testing/fstest"

	_ "github.com/effect-ts/tsgo/etscheckerhooks"
	"github.com/effect-ts/tsgo/etscore"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/bundled"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/tsoptions"
	"github.com/microsoft/typescript-go/shim/vfs/vfstest"
)

func TestEffectDiagnosticsSkipExternalLibrarySourceFiles(t *testing.T) {
	t.Parallel()

	testfs := map[string]any{
		"/src/main.ts": &fstest.MapFile{
			Data: []byte(`import { ExternalProblem } from "problem"

export class LocalProblem extends Error {}
export const value = ExternalProblem
`),
		},
		"/node_modules/problem/package.json": &fstest.MapFile{
			Data: []byte(`{"name":"problem","version":"1.0.0","types":"index.ts"}`),
		},
		"/node_modules/problem/index.ts": &fstest.MapFile{
			Data: []byte(`export class ExternalProblem extends Error {}`),
		},
	}

	fs := bundled.WrapFS(vfstest.FromMap(testfs, true))
	options := &core.CompilerOptions{
		NewLine:           core.NewLineKindLF,
		NoErrorTruncation: core.TSTrue,
		Target:            core.ScriptTargetESNext,
		Module:            core.ModuleKindCommonJS,
		ModuleResolution:  core.ModuleResolutionKindNode10,
		Effect: &etscore.EffectPluginOptions{
			Diagnostics: true,
			DiagnosticSeverity: map[string]etscore.Severity{
				"extendsNativeError": etscore.SeverityError,
			},
		},
	}
	program := compiler.NewProgram(compiler.ProgramOptions{
		Config: &tsoptions.ParsedCommandLine{
			ParsedConfig: &core.ParsedOptions{
				CompilerOptions: options,
				FileNames:       []string{"/src/main.ts"},
			},
		},
		Host:           compiler.NewCompilerHost("/", fs, bundled.LibPath(), nil, nil),
		SingleThreaded: core.TSTrue,
	})

	dependencySource := program.GetSourceFile("/node_modules/problem/index.ts")
	if dependencySource == nil {
		t.Fatal("expected dependency source file to be resolved")
	}
	if !program.IsSourceFileFromExternalLibrary(dependencySource) {
		t.Fatal("expected dependency source file to be marked as external library")
	}

	diagnostics := program.GetSemanticDiagnostics(context.Background(), nil)
	localEffectDiagnostics := 0
	externalEffectDiagnostics := 0
	for _, diag := range diagnostics {
		if !isEffectDiagnostic(diag) || diag.File() == nil {
			continue
		}
		fileName := diag.File().FileName()
		switch {
		case strings.Contains(fileName, "/node_modules/"):
			externalEffectDiagnostics++
		case fileName == "/src/main.ts":
			localEffectDiagnostics++
		}
	}

	if externalEffectDiagnostics != 0 {
		t.Fatalf("expected no Effect diagnostics from external libraries, got %d", externalEffectDiagnostics)
	}
	if localEffectDiagnostics == 0 {
		t.Fatal("expected Effect diagnostics for project source file")
	}
}

func isEffectDiagnostic(diag *ast.Diagnostic) bool {
	code := diag.Code()
	return code >= 377000 && code < 378000
}
