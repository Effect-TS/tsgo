package effecttest

import (
	"context"
	"fmt"
	"strings"
	"testing"
	"testing/fstest"

	_ "github.com/effect-ts/tsgo/etscheckerhooks"
	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/bundled"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/parser"
	"github.com/microsoft/typescript-go/shim/tsoptions"
	"github.com/microsoft/typescript-go/shim/tspath"
	"github.com/microsoft/typescript-go/shim/vfs/vfstest"
)

func TestEffectInFailureCanTriggerPluginOnlyTS2589(t *testing.T) {
	t.Parallel()

	withRule := collectDiagnosticStringsFromContent(t, buildEffectInFailureTS2589Case("enabled"))
	withoutRule := collectDiagnosticStringsFromContent(t, buildEffectInFailureTS2589Case("rule-off"))
	withoutPlugin := collectDiagnosticStringsFromContent(t, buildEffectInFailureTS2589Case("plugin-off"))

	if !hasDiagnosticCode(withRule, "TS2589:") {
		t.Fatalf("expected TS2589 with effectInFailure enabled, got %v", withRule)
	}
	if !hasDiagnosticCode(withRule, "TS377054:") {
		t.Fatalf("expected effectInFailure diagnostic with rule enabled, got %v", withRule)
	}
	if hasDiagnosticCode(withoutRule, "TS2589:") {
		t.Fatalf("did not expect TS2589 with effectInFailure disabled, got %v", withoutRule)
	}
	if hasDiagnosticCode(withoutPlugin, "TS2589:") {
		t.Fatalf("did not expect TS2589 with plugin disabled, got %v", withoutPlugin)
	}
}

func TestIssue362RemedaChunkDoesNotReportExcessiveTypeDepth(t *testing.T) {
	t.Parallel()

	withPlugin := collectDiagnosticStringsFromContent(t, buildIssue362RemedaChunkCase(true))
	withoutPlugin := collectDiagnosticStringsFromContent(t, buildIssue362RemedaChunkCase(false))

	for _, prefix := range []string{"TS2321:", "TS2589:"} {
		if hasDiagnosticCode(withoutPlugin, prefix) {
			t.Fatalf("did not expect %s without plugin, got %v", prefix, withoutPlugin)
		}
		if hasDiagnosticCode(withPlugin, prefix) {
			t.Fatalf("did not expect plugin to introduce %s, got %v", prefix, withPlugin)
		}
	}
}

func buildIssue362RemedaChunkCase(pluginEnabled bool) string {
	pluginConfig := "{}"
	if pluginEnabled {
		pluginConfig = `{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}`
	}

	return `// @filename: tsconfig.json
` + pluginConfig + `

// @filename: test.ts
type IsAny<T> = 0 extends 1 & NoInfer<T> ? true : false
type IsNever<T> = [T] extends [never] ? true : false
type If<Type extends boolean, Then, Else> = IsNever<Type> extends true ? Else : Type extends true ? Then : Else
type IsNegative<T extends number> = T extends 0 ? false : ` + "`${T}`" + ` extends ` + "`-${string}`" + ` ? true : false
type TupleOf<Length extends number, Fill = unknown> = If<IsAny<Length>, Fill[], If<IsNever<Length>, [], _TupleOf<If<IsNegative<Length>, 0, Length>, Fill, []>>>
type _TupleOf<Length extends number, Fill, Accumulator extends readonly unknown[]> = number extends Length ? Fill[] : Length extends Accumulator["length"] ? Accumulator : _TupleOf<Length, Fill, [...Accumulator, Fill]>
type IntRange<From extends number, To extends number, List extends unknown[] = TupleOf<From, never>, End extends unknown[] = TupleOf<To>> = List["length"] extends To ? Exclude<List[number], never> : IntRange<From, To, [...List, List["length"]], End>
type IntRangeInclusive<From extends number, To extends number> = IntRange<From, To> | To
type NTuple<T, N extends number, Result extends unknown[] = []> = Result["length"] extends N ? Result : NTuple<T, N, [...Result, T]>
type Chunk<T extends readonly unknown[], N extends number> = T extends readonly [] ? [] : NTuple<T[number], N>[] | { [K in IntRangeInclusive<1, N>]: NTuple<T[number], K> }[IntRangeInclusive<1, N>]

declare function chunk<T extends readonly unknown[], N extends number>(array: T, size: N): Chunk<T, N>
declare function chunk<N extends number>(size: N): <T extends readonly unknown[]>(array: T) => Chunk<T, N>
declare const items: Array<string>
const result = chunk(items, 2)
`
}

func buildEffectInFailureTS2589Case(mode string) string {
	pluginConfig := "{}"
	switch mode {
	case "enabled":
		pluginConfig = `{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}`
	case "rule-off":
		pluginConfig = `{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "effectInFailure": "off"
        }
      }
    ]
  }
}`
	}

	deep := "string"
	for range 25 {
		deep = "Wrap<" + deep + ">"
	}

	var sb strings.Builder
	sb.WriteString("// @filename: tsconfig.json\n")
	sb.WriteString(pluginConfig)
	sb.WriteString("\n\n// @filename: test.ts\n")
	sb.WriteString("import { Data, Effect } from \"effect\"\n\n")
	sb.WriteString("type TE<T> = Data.TaggedEnum<{\n")
	sb.WriteString("  A: { a: T }\n")
	sb.WriteString("  B: { b?: T }\n")
	sb.WriteString("}>\n\n")
	sb.WriteString("interface TEDefinition extends Data.TaggedEnum.WithGenerics<1> {\n")
	sb.WriteString("  readonly taggedEnum: TE<this[\"A\"]>\n")
	sb.WriteString("}\n\n")
	sb.WriteString("type Wrap<T> = Data.TaggedEnum.Kind<TEDefinition, T>\n\n")
	sb.WriteString("export const program = Effect.try({\n")
	sb.WriteString("  try: () => 1,\n")
	fmt.Fprintf(&sb, "  catch: () => Effect.fail<%s>(undefined as any)\n", deep)
	sb.WriteString("})\n")
	return sb.String()
}

func hasDiagnosticCode(diags []string, prefix string) bool {
	for _, diag := range diags {
		if strings.Contains(diag, prefix) {
			return true
		}
	}
	return false
}

func collectDiagnosticStringsFromContent(t testing.TB, content string) []string {
	t.Helper()
	AcquireProgram()
	defer ReleaseProgram()

	units := parseTestUnits(content, "test.ts")
	currentDirectory := "/.src"
	testfs := make(map[string]any)
	if err := bundledeffect.MountEffect(bundledeffect.EffectV4, testfs); err != nil {
		t.Fatal(err)
	}

	var programFileNames []string
	var tsConfigFile *tsoptions.TsConfigSourceFile
	var tsConfigUnit *testUnit
	for _, unit := range units {
		unitName := tspath.GetNormalizedAbsolutePath(unit.name, currentDirectory)
		testfs[unitName] = &fstest.MapFile{Data: []byte(unit.content)}
		if isHarnessConfigFile(unit.name) {
			path := tspath.ToPath(unitName, currentDirectory, true)
			configJSON := parser.ParseSourceFile(ast.SourceFileParseOptions{FileName: unitName, Path: path}, unit.content, core.ScriptKindJSON)
			tsConfigFile = &tsoptions.TsConfigSourceFile{SourceFile: configJSON}
			tsConfigUnit = unit
			continue
		}
		programFileNames = append(programFileNames, unitName)
	}

	fs := vfstest.FromMap(testfs, true)
	fs = bundled.WrapFS(fs)

	compilerOptions := &core.CompilerOptions{
		NewLine:                      core.NewLineKindLF,
		SkipDefaultLibCheck:          core.TSTrue,
		SkipLibCheck:                 core.TSTrue,
		NoErrorTruncation:            core.TSTrue,
		Target:                       core.ScriptTargetESNext,
		Module:                       core.ModuleKindNodeNext,
		ModuleResolution:             core.ModuleResolutionKindNodeNext,
		ESModuleInterop:              core.TSTrue,
		AllowSyntheticDefaultImports: core.TSTrue,
	}

	var parsedConfig *tsoptions.ParsedCommandLine
	if tsConfigFile != nil {
		configDir := tspath.GetNormalizedAbsolutePath(tspath.GetDirectoryPath(tsConfigUnit.name), currentDirectory)
		parseHost := &vfsParseConfigHost{fs: fs, currentDirectory: currentDirectory}
		parsedConfig = tsoptions.ParseJsonSourceFileConfigFileContent(
			tsConfigFile,
			parseHost,
			configDir,
			nil,
			nil,
			tsConfigFile.SourceFile.FileName(),
			nil,
			nil,
			nil,
		)
		if parsedConfig.CompilerOptions() != nil {
			parsedConfig.CompilerOptions().NewLine = core.NewLineKindLF
			parsedConfig.CompilerOptions().SkipDefaultLibCheck = core.TSTrue
			parsedConfig.CompilerOptions().SkipLibCheck = core.TSTrue
			parsedConfig.CompilerOptions().NoErrorTruncation = core.TSTrue
			if parsedConfig.CompilerOptions().Target == core.ScriptTargetNone {
				parsedConfig.CompilerOptions().Target = core.ScriptTargetESNext
			}
			if parsedConfig.CompilerOptions().Module == core.ModuleKindNone {
				parsedConfig.CompilerOptions().Module = core.ModuleKindNodeNext
			}
			if parsedConfig.CompilerOptions().ModuleResolution == core.ModuleResolutionKindUnknown {
				parsedConfig.CompilerOptions().ModuleResolution = core.ModuleResolutionKindNodeNext
			}
			compilerOptions = parsedConfig.CompilerOptions()
		}
	}

	host := &cachingCompilerHost{
		CompilerHost: compiler.NewCompilerHost(currentDirectory, fs, bundled.LibPath(), nil, nil),
		version:      bundledeffect.EffectV4,
	}

	var configFile *tsoptions.TsConfigSourceFile
	if parsedConfig != nil {
		configFile = parsedConfig.ConfigFile
	}
	program := compiler.NewProgram(compiler.ProgramOptions{
		Config: &tsoptions.ParsedCommandLine{
			ParsedConfig: &core.ParsedOptions{
				CompilerOptions: compilerOptions,
				FileNames:       programFileNames,
			},
			ConfigFile: configFile,
		},
		Host:           host,
		SingleThreaded: core.TSTrue,
	})

	ctx := context.Background()
	var diagnostics []*ast.Diagnostic
	diagnostics = append(diagnostics, program.GetProgramDiagnostics()...)
	diagnostics = append(diagnostics, program.GetSyntacticDiagnostics(ctx, nil)...)
	diagnostics = append(diagnostics, program.GetSemanticDiagnostics(ctx, nil)...)
	diagnostics = append(diagnostics, program.GetGlobalDiagnostics(ctx)...)

	results := make([]string, 0, len(diagnostics))
	for _, diag := range diagnostics {
		results = append(results, fmt.Sprintf("TS%d: %s", diag.Code(), diag.String()))
	}
	return results
}

func isHarnessConfigFile(fileName string) bool {
	return strings.HasSuffix(fileName, "/tsconfig.json") || fileName == "tsconfig.json"
}
