package effectconfigcheck_test

import (
	"testing"
	"testing/fstest"

	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/parser"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/TypeScript/tsc/shim/vfs"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"

	// etscheckerhooks registers the config-validation callbacks in its init, which
	// is the same path the compiler uses. Registering from the tests themselves
	// would race, because every test here runs in parallel.
	_ "github.com/effect-ts/tsgo/etscheckerhooks"
)

const currentDirectory = "/.src"

const unknownRuleNameCode = 377134

const didYouMeanCode = 377135

type parseConfigHost struct {
	fs vfs.FS
}

func (h *parseConfigHost) FS() vfs.FS { return h.fs }

func (h *parseConfigHost) GetCurrentDirectory() string { return currentDirectory }

// configDiagnostics parses the named tsconfig out of files and returns the
// diagnostics the config parse produced.
func configDiagnostics(t *testing.T, files map[string]string, configName string) []*ast.Diagnostic {
	t.Helper()

	testfs := make(map[string]any, len(files))
	for name, content := range files {
		testfs[tspath.GetNormalizedAbsolutePath(name, currentDirectory)] = &fstest.MapFile{Data: []byte(content)}
	}
	fs := vfstest.FromMap(testfs, true /*useCaseSensitiveFileNames*/)

	configFileName := tspath.GetNormalizedAbsolutePath(configName, currentDirectory)
	sourceFile := parser.ParseSourceFile(ast.SourceFileParseOptions{
		FileName: configFileName,
		Path:     tspath.ToPath(configFileName, currentDirectory, true),
	}, files[configName], core.ScriptKindJSON)

	parsed := tsoptions.ParseJsonSourceFileConfigFileContent(
		&tsoptions.TsConfigSourceFile{SourceFile: sourceFile},
		&parseConfigHost{fs: fs},
		tspath.GetDirectoryPath(configFileName),
		nil,
		nil,
		configFileName,
		nil,
		nil,
		nil,
	)
	// TS18003 (no inputs found) is inherent to a config fixture that ships no
	// source files and is not what these tests are about.
	var effectDiags []*ast.Diagnostic
	for _, diag := range parsed.Errors {
		if rule.IsEffectCode(diag.Code()) {
			effectDiags = append(effectDiags, diag)
		}
	}
	return effectDiags
}

func pluginConfig(body string) string {
	return `{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        ` + body + `
      }
    ]
  }
}`
}

func codesOf(diags []*ast.Diagnostic) []int32 {
	codes := make([]int32, 0, len(diags))
	for _, diag := range diags {
		codes = append(codes, diag.Code())
	}
	return codes
}

func TestUnknownRuleNameIsReported(t *testing.T) {
	t.Parallel()

	diags := configDiagnostics(t, map[string]string{
		"tsconfig.json": pluginConfig(`"diagnosticSeverity": { "importFromBarrel": "error" }`),
	}, "tsconfig.json")

	if len(diags) != 1 {
		t.Fatalf("expected 1 diagnostic, got %d: %v", len(diags), codesOf(diags))
	}
	if got := diags[0].Code(); got != unknownRuleNameCode {
		t.Errorf("code = %d, want %d", got, unknownRuleNameCode)
	}
	if got := diags[0].Category(); got != tsdiag.CategoryWarning {
		t.Errorf("category = %v, want warning", got)
	}
	if got := diags[0].File(); got == nil {
		t.Error("diagnostic is not anchored on the config file")
	}
	if args := diags[0].MessageArgs(); len(args) != 1 || args[0] != "importFromBarrel" {
		t.Errorf("message args = %v, want [importFromBarrel]", args)
	}
}

func TestKnownNamesAreSilent(t *testing.T) {
	t.Parallel()

	diags := configDiagnostics(t, map[string]string{
		"tsconfig.json": pluginConfig(`"diagnosticSeverity": { "floatingEffect": "error", "unusedDirective": "warning", "unknownRuleName": "warning" }`),
	}, "tsconfig.json")

	if len(diags) != 0 {
		t.Fatalf("expected no diagnostics, got %v", codesOf(diags))
	}
}

func TestOverridesAreChecked(t *testing.T) {
	t.Parallel()

	diags := configDiagnostics(t, map[string]string{
		"tsconfig.json": pluginConfig(`"overrides": [
          { "include": ["src/**"], "options": { "diagnosticSeverity": { "notARule": "error" } } }
        ]`),
	}, "tsconfig.json")

	if len(diags) != 1 {
		t.Fatalf("expected 1 diagnostic, got %d: %v", len(diags), codesOf(diags))
	}
	if args := diags[0].MessageArgs(); len(args) != 1 || args[0] != "notARule" {
		t.Errorf("message args = %v, want [notARule]", args)
	}
}

func TestNearMissSuggestsTheIntendedRule(t *testing.T) {
	t.Parallel()

	diags := configDiagnostics(t, map[string]string{
		"tsconfig.json": pluginConfig(`"diagnosticSeverity": { "floatingEfect": "error" }`),
	}, "tsconfig.json")

	if len(diags) != 1 {
		t.Fatalf("expected 1 diagnostic, got %d: %v", len(diags), codesOf(diags))
	}
	if got := diags[0].Code(); got != didYouMeanCode {
		t.Fatalf("code = %d, want %d", got, didYouMeanCode)
	}
	if args := diags[0].MessageArgs(); len(args) != 2 || args[1] != "floatingEffect" {
		t.Errorf("message args = %v, want suggestion floatingEffect", args)
	}
}

func TestSeverityIsConfigurable(t *testing.T) {
	t.Parallel()

	t.Run("off suppresses the diagnostic", func(t *testing.T) {
		t.Parallel()
		diags := configDiagnostics(t, map[string]string{
			"tsconfig.json": pluginConfig(`"diagnosticSeverity": { "importFromBarrel": "error", "unknownRuleName": "off" }`),
		}, "tsconfig.json")
		if len(diags) != 0 {
			t.Fatalf("expected no diagnostics, got %v", codesOf(diags))
		}
	})

	t.Run("error raises the category", func(t *testing.T) {
		t.Parallel()
		diags := configDiagnostics(t, map[string]string{
			"tsconfig.json": pluginConfig(`"diagnosticSeverity": { "importFromBarrel": "error", "unknownRuleName": "error" }`),
		}, "tsconfig.json")
		if len(diags) != 1 {
			t.Fatalf("expected 1 diagnostic, got %v", codesOf(diags))
		}
		if got := diags[0].Category(); got != tsdiag.CategoryError {
			t.Errorf("category = %v, want error", got)
		}
	})
}

func TestUnrelatedPluginIsIgnored(t *testing.T) {
	t.Parallel()

	diags := configDiagnostics(t, map[string]string{
		"tsconfig.json": `{
  "compilerOptions": {
    "plugins": [
      { "name": "some-other-plugin", "diagnosticSeverity": { "importFromBarrel": "error" } }
    ]
  }
}`,
	}, "tsconfig.json")

	if len(diags) != 0 {
		t.Fatalf("expected no diagnostics, got %v", codesOf(diags))
	}
}

func TestExtendedConfigIsChecked(t *testing.T) {
	t.Parallel()

	diags := configDiagnostics(t, map[string]string{
		"tsconfig.base.json": pluginConfig(`"diagnosticSeverity": { "importFromBarrel": "error" }`),
		"tsconfig.json":      `{ "extends": "./tsconfig.base.json" }`,
	}, "tsconfig.json")

	if len(diags) != 1 {
		t.Fatalf("expected 1 diagnostic, got %d: %v", len(diags), codesOf(diags))
	}
	if file := diags[0].File(); file == nil || tspath.GetBaseFileName(file.FileName()) != "tsconfig.base.json" {
		t.Errorf("diagnostic is not anchored on the config that declares the plugin block")
	}
}

func TestDiagnosticsDisabledSuppressesTheCheck(t *testing.T) {
	t.Parallel()

	diags := configDiagnostics(t, map[string]string{
		"tsconfig.json": pluginConfig(`"diagnostics": false, "diagnosticSeverity": { "importFromBarrel": "error" }`),
	}, "tsconfig.json")

	if len(diags) != 0 {
		t.Fatalf("expected no diagnostics, got %v", codesOf(diags))
	}
}

func TestInheritedSuppressionAcrossExtends(t *testing.T) {
	t.Parallel()

	t.Run("unknownRuleName off in a base silences a name declared in a child", func(t *testing.T) {
		t.Parallel()
		diags := configDiagnostics(t, map[string]string{
			"tsconfig.base.json": pluginConfig(`"diagnosticSeverity": { "unknownRuleName": "off" }`),
			"tsconfig.json": `{ "extends": "./tsconfig.base.json", "compilerOptions": { "plugins": [
			  { "name": "@effect/language-service", "diagnosticSeverity": { "importFromBarrel": "error" } }
			] } }`,
		}, "tsconfig.json")
		if len(diags) != 0 {
			t.Fatalf("expected no diagnostics, got %v", codesOf(diags))
		}
	})

	t.Run("diagnostics false in a base silences a name declared in a child", func(t *testing.T) {
		t.Parallel()
		diags := configDiagnostics(t, map[string]string{
			"tsconfig.base.json": pluginConfig(`"diagnostics": false`),
			"tsconfig.json": `{ "extends": "./tsconfig.base.json", "compilerOptions": { "plugins": [
			  { "name": "@effect/language-service", "diagnosticSeverity": { "importFromBarrel": "error" } }
			] } }`,
		}, "tsconfig.json")
		if len(diags) != 0 {
			t.Fatalf("expected no diagnostics, got %v", codesOf(diags))
		}
	})

	t.Run("a child silences a name declared in a base it does not own", func(t *testing.T) {
		t.Parallel()
		diags := configDiagnostics(t, map[string]string{
			"tsconfig.base.json": pluginConfig(`"diagnosticSeverity": { "importFromBarrel": "error" }`),
			"tsconfig.json": `{ "extends": "./tsconfig.base.json", "compilerOptions": { "plugins": [
			  { "name": "@effect/language-service", "diagnosticSeverity": { "unknownRuleName": "off" } }
			] } }`,
		}, "tsconfig.json")
		if len(diags) != 0 {
			t.Fatalf("expected no diagnostics, got %v", codesOf(diags))
		}
	})

	t.Run("a child raises the severity of a name declared in a base", func(t *testing.T) {
		t.Parallel()
		diags := configDiagnostics(t, map[string]string{
			"tsconfig.base.json": pluginConfig(`"diagnosticSeverity": { "importFromBarrel": "error" }`),
			"tsconfig.json": `{ "extends": "./tsconfig.base.json", "compilerOptions": { "plugins": [
			  { "name": "@effect/language-service", "diagnosticSeverity": { "unknownRuleName": "error" } }
			] } }`,
		}, "tsconfig.json")
		if len(diags) != 1 {
			t.Fatalf("expected 1 diagnostic, got %v", codesOf(diags))
		}
		if got := diags[0].Category(); got != tsdiag.CategoryError {
			t.Errorf("category = %v, want error", got)
		}
		if file := diags[0].File(); file == nil || tspath.GetBaseFileName(file.FileName()) != "tsconfig.base.json" {
			t.Error("diagnostic should stay anchored on the config that declares the name")
		}
	})
}
