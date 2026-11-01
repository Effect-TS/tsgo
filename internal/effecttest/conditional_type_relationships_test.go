package effecttest

import (
	"context"
	_ "embed"
	"fmt"
	"slices"
	"strings"
	"testing"
	"testing/fstest"

	_ "github.com/effect-ts/tsgo/etscheckerhooks"
	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/bundled"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/parser"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

//go:embed testdata/conditionalTypeRelationships.ts
var conditionalTypeRelationshipsSource string

func TestConditionalTypeRelationships(t *testing.T) {
	t.Parallel()

	for _, mode := range []string{"plugin-off", "rule-off", "enabled"} {
		t.Run(mode, func(t *testing.T) {
			t.Parallel()
			severity := "off"
			if mode == "enabled" {
				severity = "error"
			}
			source := strings.ReplaceAll(conditionalTypeRelationshipsSource, "effectInFailure:CONTROL", "effectInFailure:"+severity)
			units := parseTestUnits(source, "conditionalTypeRelationships.ts")

			// Match the entire diagnostic set, including declarations and dependencies.
			// Every positive regression above must remain free of extra diagnostics.
			want := []string{
				conditionalRelationshipExpected(t, units, "returning.d.ts", 2430, "BrokenReturning extends", "BrokenReturning"),
				conditionalRelationshipExpected(t, units, "invalid.ts", 2322, "badAssignment: number", "badAssignment"),
				conditionalRelationshipExpected(t, units, "invalid.ts", 2322, "badNarrowed: T extends", "badNarrowed"),
			}
			if mode == "enabled" {
				want = append(want, conditionalRelationshipExpected(t, units, "effectControl.ts", 377054,
					"Effect.try({", "Effect.try({\n  try: () => 1,\n  catch: () => Effect.logError(\"boom\"),\n})"))
			}
			got, details := collectConditionalRelationshipDiagnostics(t, units, mode != "plugin-off")
			slices.Sort(got)
			slices.Sort(want)
			if !slices.Equal(got, want) {
				t.Fatalf("diagnostic locations = %v, want %v\n%s", got, want, strings.Join(details, "\n"))
			}
		})
	}
}

// Find an exact source span without hard-coding line numbers that drift when the
// shared fixture changes. All expected spans are ASCII, so byte and UTF-16 lengths agree.
func conditionalRelationshipExpected(t *testing.T, units []*testUnit, file string, code int, anchor, span string) string {
	t.Helper()
	for _, unit := range units {
		if unit.name != file {
			continue
		}
		if strings.Count(unit.content, anchor) != 1 {
			t.Fatalf("%s: expected one occurrence of %q", file, anchor)
		}
		pos := strings.Index(unit.content, anchor)
		if !strings.HasPrefix(unit.content[pos:], span) {
			t.Fatalf("%s: expected span %q at %q", file, span, anchor)
		}
		prefix := unit.content[:pos]
		line := strings.Count(prefix, "\n") + 1
		column := pos - strings.LastIndex(prefix, "\n")
		return fmt.Sprintf("/.src/%s(%d,%d,%d): TS%d", file, line, column, len(span), code)
	}
	t.Fatalf("missing fixture unit %s", file)
	return ""
}

// Like collectDiagnosticStringsFromContent, but preserve spans and check .d.ts
// files and default libraries. That helper unconditionally skips both.
func collectConditionalRelationshipDiagnostics(t *testing.T, units []*testUnit, pluginEnabled bool) ([]string, []string) {
	t.Helper()
	AcquireProgram()
	defer ReleaseProgram()

	const currentDirectory = "/.src"
	testfs := make(map[string]any)
	if err := bundledeffect.MountEffect(bundledeffect.EffectV4, testfs); err != nil {
		t.Fatal(err)
	}
	testfs[currentDirectory+"/package.json"] = &fstest.MapFile{Data: []byte(`{"type":"module"}`)}
	var fileNames []tspath.RootedFilePath
	for _, unit := range units {
		path := tspath.GetNormalizedAbsolutePath(unit.name, tspath.RootedDirectoryPath(currentDirectory))
		testfs[path] = &fstest.MapFile{Data: []byte(unit.content)}
		fileNames = append(fileNames, tspath.RootedFilePath(path))
	}

	plugins := "[]"
	if pluginEnabled {
		plugins = `[{"name":"@effect/language-service"}]`
	}
	configText := fmt.Sprintf(`{"compilerOptions":{
		"target":"ESNext","module":"NodeNext","moduleResolution":"NodeNext",
		"strict":true,"noEmit":true,"noErrorTruncation":true,
		"skipLibCheck":false,"skipDefaultLibCheck":false,"plugins":%s
	}}`, plugins)
	configName := tspath.RootedFilePath(currentDirectory + "/tsconfig.json")
	testfs[string(configName)] = &fstest.MapFile{Data: []byte(configText)}
	fs := bundled.WrapFS(vfstest.FromMap(testfs, tspath.CaseSensitive))
	configJSON := parser.ParseSourceFile(ast.NewSourceFileParseOptions(configName,
		tspath.PathKeyForFile(configName, true)), configText, core.ScriptKindJSON)
	config := tsoptions.ParseJsonSourceFileConfigFileContent(
		&tsoptions.TsConfigSourceFile{SourceFile: configJSON}, tsoptions.NewDefaultParseConfigHost(fs),
		tspath.RootedDirectoryPath(currentDirectory), nil, nil, nil, nil)
	programConfig := tsoptions.NewParsedCommandLine(config.CompilerOptions(), fileNames, nil,
		tspath.RootedDirectoryPath(currentDirectory), tspath.CaseSensitive)
	programConfig.ConfigFile = config.ConfigFile
	programConfig.Errors = config.Errors
	host := &cachingCompilerHost{
		CompilerHost: compiler.NewCompilerHost(fs, bundled.LibPath(), nil, nil, nil),
		version:      bundledeffect.EffectV4,
	}
	program := compiler.NewProgram(compiler.ProgramOptions{
		Config: programConfig, Host: host, SingleThreaded: core.TSTrue,
	})

	ctx := context.Background()
	var diagnostics []*ast.Diagnostic
	diagnostics = append(diagnostics, program.GetConfigFileParsingDiagnostics()...)
	diagnostics = append(diagnostics, program.GetProgramDiagnostics()...)
	diagnostics = append(diagnostics, program.GetSyntacticDiagnostics(ctx, nil)...)
	diagnostics = append(diagnostics, program.GetSemanticDiagnostics(ctx, nil)...)
	diagnostics = append(diagnostics, program.GetGlobalDiagnostics(ctx)...)
	var locations, details []string
	for _, diag := range diagnostics {
		location := "<global>"
		if file := diag.File(); file != nil && diag.Loc().Pos() >= 0 {
			line, column := scanner.GetECMALineAndUTF16CharacterOfPosition(file, diag.Loc().Pos())
			location = fmt.Sprintf("%s(%d,%d,%d)", file.FileName(), line+1, column+1, diag.Loc().Len())
		}
		location = fmt.Sprintf("%s: TS%d", location, diag.Code())
		locations = append(locations, location)
		details = append(details, location+": "+diag.String())
	}
	return locations, details
}
