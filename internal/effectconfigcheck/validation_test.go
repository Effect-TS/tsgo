package effectconfigcheck_test

import (
	"strings"
	"testing"
	"testing/fstest"

	_ "github.com/effect-ts/tsgo/etscheckerhooks"
	"github.com/effect-ts/tsgo/etscore"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/bundled"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/execute/tsc"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/TypeScript/tsc/shim/vfs"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

type parseHost struct{ fs vfs.FS }

func (h *parseHost) FS() vfs.FS                  { return h.fs }
func (h *parseHost) GetCurrentDirectory() string { return "/" }

func newHost(files map[string]string) *parseHost {
	entries := map[string]any{"/main.ts": &fstest.MapFile{Data: []byte("export {}")}}
	for name, text := range files {
		entries[name] = &fstest.MapFile{Data: []byte(text)}
	}
	return &parseHost{fs: bundled.WrapFS(vfstest.FromMap(entries, true))}
}

func parse(t *testing.T, host *parseHost, path string, cache tsoptions.ExtendedConfigCache) *tsoptions.ParsedCommandLine {
	t.Helper()
	text, ok := host.fs.ReadFile(path)
	if !ok {
		t.Fatalf("missing config %s", path)
	}
	source := tsoptions.NewTsconfigSourceFileFromFilePath(path, tspath.Path(path), text)
	return tsoptions.ParseJsonSourceFileConfigFileContent(source, host, tspath.GetDirectoryPath(path), nil, nil, path, nil, nil, cache)
}

func config(options string, extra string) string {
	return `{"files":["/main.ts"],"compilerOptions":{"plugins":[{"name":"@effect/language-service",` + options + `}]}` + extra + `}`
}

func unknownDiagnostics(config *tsoptions.ParsedCommandLine) []*ast.Diagnostic {
	var result []*ast.Diagnostic
	for _, diagnostic := range config.Errors {
		if diagnostic.Code() == 377134 {
			result = append(result, diagnostic)
		}
	}
	return result
}

func TestUnknownRuleNames(t *testing.T) {
	t.Parallel()
	for _, tt := range []struct {
		name, options string
		count         int
		category      diagnostics.Category
	}{
		{"typo", `"diagnosticSeverity":{"floatingEfect":"error"}`, 1, diagnostics.CategoryWarning},
		{"unknown disabled rule still invalid", `"diagnosticSeverity":{"floatingEfect":"off"}`, 1, diagnostics.CategoryWarning},
		{"known names", `"diagnosticSeverity":{"floatingEffect":"error","unusedDirective":"warning","unknownRuleName":"warning"}`, 0, diagnostics.CategoryWarning},
		{"error", `"diagnosticSeverity":{"floatingEfect":"error","unknownRuleName":"error"}`, 1, diagnostics.CategoryError},
		{"suggestion", `"diagnosticSeverity":{"floatingEfect":"error","unknownRuleName":"suggestion"}`, 1, diagnostics.CategorySuggestion},
		{"off", `"diagnosticSeverity":{"floatingEfect":"error","unknownRuleName":"off"}`, 0, diagnostics.CategoryWarning},
		{"disabled", `"diagnostics":false,"diagnosticSeverity":{"floatingEfect":"error"}`, 0, diagnostics.CategoryWarning},
		{"null", `"diagnosticSeverity":null,"overrides":[{"options":{"diagnosticSeverity":{"floatingEfect":"error"}}}]`, 0, diagnostics.CategoryWarning},
		{"override", `"overrides":[null,{"options":{"diagnosticSeverity":{"floatingEfect":"error"}}}]`, 1, diagnostics.CategoryWarning},
	} {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			host := newHost(map[string]string{"/tsconfig.json": config(tt.options, "")})
			parsed := parse(t, host, "/tsconfig.json", nil)
			got := unknownDiagnostics(parsed)
			if len(got) != tt.count {
				t.Fatalf("want %d warnings, got %d: %v", tt.count, len(got), parsed.Errors)
			}
			for _, diagnostic := range got {
				if diagnostic.Category() != tt.category {
					t.Fatalf("wrong category: %v", diagnostic.Category())
				}
				if diagnostic.File() == nil || diagnostic.File().FileName() != "/tsconfig.json" {
					t.Fatal("missing local config location")
				}
				if text := diagnostic.File().Text()[diagnostic.Pos():diagnostic.End()]; text != `"floatingEfect"` {
					t.Fatalf("wrong underline: %q", text)
				}
			}
		})
	}
}

func TestUnknownRuleNamesExtends(t *testing.T) {
	t.Parallel()
	for _, tt := range []struct {
		name, base, child string
		count             int
		category          diagnostics.Category
		local             bool
	}{
		{"inherited key", `"diagnosticSeverity":{"floatingEfect":"error"}`, `"diagnosticSeverity":{}`, 1, diagnostics.CategoryWarning, false},
		{"child silences base", `"diagnosticSeverity":{"floatingEfect":"error"}`, `"diagnosticSeverity":{"unknownRuleName":"off"}`, 0, diagnostics.CategoryWarning, false},
		{"child disables diagnostics", `"diagnosticSeverity":{"floatingEfect":"error"}`, `"diagnostics":false`, 0, diagnostics.CategoryWarning, false},
		{"child raises base", `"diagnosticSeverity":{"floatingEfect":"error"}`, `"diagnosticSeverity":{"unknownRuleName":"error"}`, 1, diagnostics.CategoryError, false},
		{"base silences child", `"diagnosticSeverity":{"unknownRuleName":"off"}`, `"diagnosticSeverity":{"floatingEfect":"error"}`, 0, diagnostics.CategoryWarning, true},
		{"base disables child", `"diagnostics":false`, `"diagnosticSeverity":{"floatingEfect":"error"}`, 0, diagnostics.CategoryWarning, true},
		{"base raises child", `"diagnosticSeverity":{"unknownRuleName":"error"}`, `"diagnosticSeverity":{"floatingEfect":"error"}`, 1, diagnostics.CategoryError, true},
	} {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			host := newHost(map[string]string{
				"/base.json":     config(tt.base, ""),
				"/middle.json":   `{"extends":"./base.json"}`,
				"/tsconfig.json": config(tt.child, `,"extends":"./middle.json"`),
			})
			parsed := parse(t, host, "/tsconfig.json", &tsc.ExtendedConfigCache{})
			got := unknownDiagnostics(parsed)
			if len(got) != tt.count {
				t.Fatalf("want %d warnings, got %d: %v", tt.count, len(got), parsed.Errors)
			}
			for _, d := range got {
				if d.Category() != tt.category {
					t.Fatalf("wrong category: %v", d.Category())
				}
				if (d.File() != nil) != tt.local {
					t.Fatalf("wrong inherited/local location: %v", d.File())
				}
			}
		})
	}
}

func TestInheritedOverridesUseLocalSyntaxOnly(t *testing.T) {
	t.Parallel()
	host := newHost(map[string]string{
		"/base.json":     config(`"overrides":[{"options":{"diagnosticSeverity":{"inheritedTypo":"warning"}}}]`, ""),
		"/tsconfig.json": config(`"overrides":[false,{"options":{"diagnosticSeverity":{"localTypo":"warning"}}}]`, `,"extends":"./base.json"`),
	})
	got := unknownDiagnostics(parse(t, host, "/tsconfig.json", nil))
	if len(got) != 2 {
		t.Fatalf("want two warnings, got %d", len(got))
	}
	if got[0].File() != nil {
		t.Fatal("inherited override must not be attributed to a local override")
	}
	if got[1].File() == nil {
		t.Fatal("local override must have a location")
	}
	if text := got[1].File().Text()[got[1].Pos():got[1].End()]; text != `"localTypo"` {
		t.Fatalf("wrong local underline: %q", text)
	}
}

func TestUnknownRuleNamesJSONAPI(t *testing.T) {
	t.Parallel()
	// Use the compiler's JSON representation, which both providers accept.
	raw, errors := tsoptions.ParseConfigFileTextToJson("/tsconfig.json", "/tsconfig.json", config(`"diagnosticSeverity":{"floatingEfect":"error"}`, ""))
	if len(errors) != 0 {
		t.Fatalf("invalid config fixture: %v", errors)
	}
	parsed := tsoptions.ParseJsonConfigFileContent(raw, newHost(nil), "/", nil, "/tsconfig.json", nil, nil, nil)
	got := unknownDiagnostics(parsed)
	if len(got) != 1 || got[0].File() != nil {
		t.Fatalf("expected one locationless warning: %v", got)
	}
}

func TestProjectReferencesAndSharedExtends(t *testing.T) {
	t.Parallel()
	host := newHost(map[string]string{
		"/base.json":       config(`"diagnosticSeverity":{"floatingEfect":"error"}`, ""),
		"/tsconfig.json":   `{"files":[],"references":[{"path":"./a"},{"path":"./b"},{"path":"./c"}]}`,
		"/a/tsconfig.json": config(`"diagnosticSeverity":{"unknownRuleName":"off"}`, `,"extends":"../base.json"`),
		"/b/tsconfig.json": config(`"diagnosticSeverity":{"unknownRuleName":"error"}`, `,"extends":"../base.json"`),
		"/c/tsconfig.json": `{"extends":"../base.json"}`,
	})
	cache := &tsc.ExtendedConfigCache{}
	root := parse(t, host, "/tsconfig.json", cache)
	program := compiler.NewProgram(compiler.ProgramOptions{
		Config:         root,
		Host:           compiler.NewCompilerHost("/", host.fs, bundled.LibPath(), cache, nil),
		SingleThreaded: core.TSTrue,
	})
	if got := unknownDiagnostics(root); len(got) != 0 {
		t.Fatal("reference diagnostics leaked into solution config")
	}
	refs := program.GetResolvedProjectReferences()
	if len(refs) != 3 {
		t.Fatalf("expected three resolved references, got %d", len(refs))
	}
	for i, ref := range refs {
		if ref == nil {
			t.Fatalf("reference %d did not resolve", i)
		}
		got := unknownDiagnostics(ref)
		if i == 0 {
			if len(got) != 0 {
				t.Fatal("project a did not silence inherited typo")
			}
			continue
		}
		category := diagnostics.CategoryWarning
		if i == 1 {
			category = diagnostics.CategoryError
		}
		if len(got) != 1 || got[0].Category() != category || got[0].File() != nil {
			t.Fatalf("project %d: unexpected diagnostics %v", i, got)
		}
	}
	// Parsing the base through the same cache must not reuse a child's severity.
	got := unknownDiagnostics(parse(t, host, "/base.json", cache))
	if len(got) != 1 || got[0].Category() != diagnostics.CategoryWarning {
		t.Fatalf("cached severity leaked: %v", got)
	}
}

func TestMultipleExtendsAndExistingOptions(t *testing.T) {
	t.Parallel()
	host := newHost(map[string]string{
		"/base.json":     config(`"diagnosticSeverity":{"floatingEfect":"error"}`, ""),
		"/severity.json": config(`"diagnosticSeverity":{"unknownRuleName":"error"}`, ""),
		"/tsconfig.json": `{"extends":["./base.json","./severity.json"]}`,
	})
	got := unknownDiagnostics(parse(t, host, "/tsconfig.json", nil))
	if len(got) != 1 || got[0].Category() != diagnostics.CategoryError {
		t.Fatalf("multiple extends failed: %v", got)
	}
	text, _ := host.fs.ReadFile("/tsconfig.json")
	source := tsoptions.NewTsconfigSourceFileFromFilePath("/tsconfig.json", "/tsconfig.json", text)
	existing := &core.CompilerOptions{Effect: &etscore.EffectPluginOptions{Diagnostics: true, DiagnosticSeverity: map[string]etscore.Severity{"existingTypo": etscore.SeverityError}}}
	parsed := tsoptions.ParseJsonSourceFileConfigFileContent(source, host, "/", existing, nil, "/tsconfig.json", nil, nil, nil)
	got = unknownDiagnostics(parsed)
	if len(got) != 1 || !strings.Contains(strings.Join(got[0].MessageArgs(), " "), "existingTypo") {
		t.Fatalf("existing options were not validated after merging: %v", got)
	}
}

func TestOtherPluginsAndCompilerErrors(t *testing.T) {
	t.Parallel()
	host := newHost(map[string]string{
		"/tsconfig.json": `{"files":["/main.ts"],"compilerOptions":{"strcit":true,"plugins":[{"name":"other-plugin","diagnosticSeverity":{"unrelatedRule":"error"}}]}}`,
	})
	parsed := parse(t, host, "/tsconfig.json", nil)
	if len(unknownDiagnostics(parsed)) != 0 {
		t.Fatal("validated an unrelated plugin")
	}
	if len(parsed.Errors) != 1 {
		t.Fatalf("expected the existing compiler-option diagnostic, got %v", parsed.Errors)
	}
}

func TestUnknownRuleNamesDeterministicOrder(t *testing.T) {
	t.Parallel()
	host := newHost(map[string]string{
		"/tsconfig.json": config(`"diagnosticSeverity":{"zTypo":"warning","aTypo":"error","mTypo":"off"}`, ""),
	})
	got := unknownDiagnostics(parse(t, host, "/tsconfig.json", nil))
	if len(got) != 3 {
		t.Fatalf("expected three warnings, got %d", len(got))
	}
	for i, name := range []string{"aTypo", "mTypo", "zTypo"} {
		if args := got[i].MessageArgs(); len(args) != 1 || args[0] != name {
			t.Fatalf("wrong diagnostic order: %v", args)
		}
	}
}
