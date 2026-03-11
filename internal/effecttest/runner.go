package effecttest

import (
	"context"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
	"testing/fstest"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/bundled"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/parser"
	"github.com/microsoft/typescript-go/shim/testutil/harnessutil"
	"github.com/microsoft/typescript-go/shim/tsoptions"
	"github.com/microsoft/typescript-go/shim/tspath"
	"github.com/microsoft/typescript-go/shim/vfs"
	"github.com/microsoft/typescript-go/shim/vfs/vfstest"

	"github.com/effect-ts/effect-typescript-go/internal/typeparser"

	// Import etscheckerhooks to register Effect diagnostic callbacks
	_ "github.com/effect-ts/effect-typescript-go/etscheckerhooks"
)

// TestCasesDir returns the path to the Effect test cases directory for the given version.
func TestCasesDir(version EffectVersion) string {
	return filepath.Join(EffectTsGoRootPath(), "testdata", "tests", string(version))
}

// DiscoverTestCases finds all .ts test files in the test cases directory for the given version.
func DiscoverTestCases(version EffectVersion) ([]string, error) {
	dir := TestCasesDir(version)
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, err
	}

	var cases []string
	for _, entry := range entries {
		if !entry.IsDir() && filepath.Ext(entry.Name()) == ".ts" {
			cases = append(cases, filepath.Join(dir, entry.Name()))
		}
	}
	return cases, nil
}

var lineDelimiter = regexp.MustCompile("\r?\n")
var optionRegex = regexp.MustCompile(`(?m)^\/{2}\s*@(\w+)\s*:\s*([^\r\n]*)`)

// testUnit represents a single file within a multi-file test case.
type testUnit struct {
	name    string
	content string
}

// parseTestUnits parses a test file into multiple test units based on @filename directives.
func parseTestUnits(content string, defaultFileName string) []*testUnit {
	lines := lineDelimiter.Split(content, -1)

	var units []*testUnit
	var currentContent strings.Builder
	var currentFileName string

	for _, line := range lines {
		if testMetaData := optionRegex.FindStringSubmatch(line); testMetaData != nil {
			metaDataName := strings.ToLower(testMetaData[1])
			if metaDataName == "filename" {
				// Save the current file if we have one
				if currentFileName != "" && currentContent.Len() > 0 {
					units = append(units, &testUnit{
						name:    currentFileName,
						content: currentContent.String(),
					})
				}
				// Start new file
				currentFileName = strings.TrimSpace(testMetaData[2])
				currentContent.Reset()
				continue
			}
		}
		// Add line to current file content
		if currentContent.Len() > 0 {
			currentContent.WriteRune('\n')
		}
		currentContent.WriteString(line)
	}

	// Handle the final file
	if currentFileName != "" {
		units = append(units, &testUnit{
			name:    currentFileName,
			content: currentContent.String(),
		})
	} else if currentContent.Len() > 0 {
		// Single file test
		units = append(units, &testUnit{
			name:    defaultFileName,
			content: currentContent.String(),
		})
	}

	return units
}

// defaultTsConfig is the default tsconfig content injected when a test does not provide one.
// It enables the Effect language service plugin with default diagnostic severities.
const defaultTsConfig = `{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "ignoreEffectErrorsInTscExitCode": true,
        "skipDisabledOptimization": true
      }
    ]
  }
}`

// RunEffectTest executes a single Effect diagnostic test case for the given version.
func RunEffectTest(t *testing.T, version EffectVersion, testFile string) {
	AcquireProgram()
	defer ReleaseProgram()

	// Read the test file
	content, err := os.ReadFile(testFile)
	if err != nil {
		t.Fatal("Failed to read test file:", err)
	}

	// Parse test file into units (handles @filename directives)
	defaultFileName := tspath.GetBaseFileName(testFile)
	units := parseTestUnits(string(content), defaultFileName)

	// Create the test filesystem
	testfs := make(map[string]any)

	// Mount Effect package
	if err := MountEffect(version, testfs); err != nil {
		t.Fatal("Failed to mount Effect:", err)
	}

	// Current directory for the test
	currentDirectory := "/.src"

	// Add test files to VFS
	var inputFiles []*harnessutil.TestFile
	var tsConfigFile *tsoptions.TsConfigSourceFile
	var tsConfigUnit *testUnit
	tsconfigInjected := false

	for _, unit := range units {
		unitName := tspath.GetNormalizedAbsolutePath(unit.name, currentDirectory)
		testfs[unitName] = &fstest.MapFile{
			Data: []byte(unit.content),
		}

		// Files placed under /node_modules/ via @filename directives are
		// VFS-only: they set up synthetic packages but are not added to the
		// program's root file list.  The compiler discovers them through
		// module resolution when test source files import from them.
		if strings.HasPrefix(unitName, "/node_modules/") {
			continue
		}

		// Check if this is a tsconfig.json file
		if harnessutil.GetConfigNameFromFileName(unit.name) != "" {
			// Parse tsconfig
			path := tspath.ToPath(unitName, currentDirectory, true)
			configJson := parser.ParseSourceFile(ast.SourceFileParseOptions{
				FileName: unitName,
				Path:     path,
			}, unit.content, core.ScriptKindJSON)
			tsConfigFile = &tsoptions.TsConfigSourceFile{
				SourceFile: configJson,
			}
			tsConfigUnit = unit
		} else {
			inputFiles = append(inputFiles, &harnessutil.TestFile{
				UnitName: unitName,
				Content:  unit.content,
			})
		}
	}

	// Inject default tsconfig if none was provided by the test
	if tsConfigUnit == nil {
		tsconfigInjected = true
		tsConfigUnit = &testUnit{
			name:    "tsconfig.json",
			content: defaultTsConfig,
		}
		unitName := tspath.GetNormalizedAbsolutePath(tsConfigUnit.name, currentDirectory)
		testfs[unitName] = &fstest.MapFile{
			Data: []byte(tsConfigUnit.content),
		}
		path := tspath.ToPath(unitName, currentDirectory, true)
		configJson := parser.ParseSourceFile(ast.SourceFileParseOptions{
			FileName: unitName,
			Path:     path,
		}, tsConfigUnit.content, core.ScriptKindJSON)
		tsConfigFile = &tsoptions.TsConfigSourceFile{
			SourceFile: configJson,
		}
	}

	// Create VFS
	fs := vfstest.FromMap(testfs, true /*useCaseSensitiveFileNames*/)
	fs = bundled.WrapFS(fs)
	fs = harnessutil.NewOutputRecorderFS(fs)

	// Setup compiler options
	compilerOptions := &core.CompilerOptions{
		NewLine:             core.NewLineKindLF,
		SkipDefaultLibCheck: core.TSTrue,
		NoErrorTruncation:   core.TSTrue,
		Target:              core.ScriptTargetESNext,
		Module:              core.ModuleKindNodeNext,
		ModuleResolution:    core.ModuleResolutionKindNodeNext,
		// Enable esModuleInterop for Effect
		ESModuleInterop:              core.TSTrue,
		AllowSyntheticDefaultImports: core.TSTrue,
	}

	var parsedConfig *tsoptions.ParsedCommandLine

	// Parse tsconfig if present
	if tsConfigFile != nil {
		configDir := tspath.GetDirectoryPath(tsConfigUnit.name)
		configDir = tspath.GetNormalizedAbsolutePath(configDir, currentDirectory)

		// Create a simple parse host using our VFS
		parseHost := &vfsParseConfigHost{
			fs:               fs,
			currentDirectory: currentDirectory,
		}

		parsedConfig = tsoptions.ParseJsonSourceFileConfigFileContent(
			tsConfigFile,
			parseHost,
			configDir,
			nil, // existingOptions
			nil, // existingOptionsRaw
			tsConfigFile.SourceFile.FileName(),
			nil, // resolutionStack
			nil, // extraFileExtensions
			nil, // extendedConfigCache
		)
		// Use parsed compiler options
		if parsedConfig.CompilerOptions() != nil {
			// Merge with our defaults
			parsedConfig.CompilerOptions().NewLine = core.NewLineKindLF
			parsedConfig.CompilerOptions().SkipDefaultLibCheck = core.TSTrue
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

	// Get file names for compilation
	var programFileNames []string
	for _, file := range inputFiles {
		programFileNames = append(programFileNames, file.UnitName)
	}

	// Create compiler host with AST caching for package files
	host := &cachingCompilerHost{
		CompilerHost: compiler.NewCompilerHost(currentDirectory, fs, bundled.LibPath(), nil, nil),
		version:      version,
	}

	// Create program
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

	// Get diagnostics
	ctx := context.Background()
	var diagnostics []*ast.Diagnostic
	diagnostics = append(diagnostics, program.GetProgramDiagnostics()...)
	diagnostics = append(diagnostics, program.GetSyntacticDiagnostics(ctx, nil)...)
	diagnostics = append(diagnostics, program.GetSemanticDiagnostics(ctx, nil)...)
	diagnostics = append(diagnostics, program.GetGlobalDiagnostics(ctx)...)

	// Prepare files for baseline
	var allFiles []*harnessutil.TestFile
	if tsConfigUnit != nil && !tsconfigInjected {
		allFiles = append(allFiles, &harnessutil.TestFile{
			UnitName: tspath.GetNormalizedAbsolutePath(tsConfigUnit.name, currentDirectory),
			Content:  tsConfigUnit.content,
		})
	}
	allFiles = append(allFiles, inputFiles...)

	// Generate error baseline
	baselineName := strings.TrimSuffix(tspath.GetBaseFileName(testFile), ".ts")
	baselineSubfolder := string(version)

	t.Run("errors", func(t *testing.T) {
		c, done := program.GetTypeChecker(ctx)
		defer done()
		effectVersion := typeparser.DetectEffectVersionString(c)
		DoEffectErrorBaseline(
			t,
			baselineName,
			allFiles,
			diagnostics,
			false, // pretty
			baselineSubfolder,
			effectVersion,
		)
	})

	// Generate piping flow baseline
	t.Run("pipings", func(t *testing.T) {
		c, done := program.GetTypeChecker(ctx)
		defer done()
		DoPipingFlowBaseline(
			t,
			baselineName,
			c,
			inputFiles,
			func(fileName string) *ast.SourceFile {
				return program.GetSourceFile(fileName)
			},
			baselineSubfolder,
		)
	})

	// Generate layer graph baseline
	t.Run("layers", func(t *testing.T) {
		c, done := program.GetTypeChecker(ctx)
		defer done()
		DoLayerGraphBaseline(
			t,
			baselineName,
			c,
			inputFiles,
			func(fileName string) *ast.SourceFile {
				return program.GetSourceFile(fileName)
			},
			baselineSubfolder,
		)
	})
}

// vfsParseConfigHost implements tsoptions.ParseConfigHost for VFS.
type vfsParseConfigHost struct {
	fs               vfs.FS
	currentDirectory string
}

func (h *vfsParseConfigHost) FS() vfs.FS {
	return h.fs
}

func (h *vfsParseConfigHost) GetCurrentDirectory() string {
	return h.currentDirectory
}
