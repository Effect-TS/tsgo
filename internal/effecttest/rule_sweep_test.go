package effecttest

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
)

// Rule sweep stress tests (originally built while investigating
// https://github.com/Effect-TS/tsgo/issues/301).
//
// anyUnknownInErrorContext and effectInFailure are the only rules that request
// the type of every node in a file, which exercises checker paths regular
// type checking never touches. These tests run both rules (plus all
// default-enabled rules) over broad code corpora and fail if any input makes
// diagnostics collection panic or fail to terminate within a generous
// watchdog timeout.

const ruleSweepPluginConfig = `{
  "compilerOptions": {
    "strict": true,
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "anyUnknownInErrorContext": "error",
          "effectInFailure": "error"
        }
      }
    ]
  }
}`

const ruleSweepTimeout = 120 * time.Second

// runRuleSweepCase collects diagnostics for the given test content under a
// watchdog: it fails the test if collection panics or exceeds ruleSweepTimeout.
func runRuleSweepCase(t *testing.T, content string) time.Duration {
	t.Helper()

	start := time.Now()
	done := make(chan []string, 1)
	panicked := make(chan any, 1)
	go func() {
		defer func() {
			if r := recover(); r != nil {
				panicked <- r
			}
		}()
		done <- collectDiagnosticStringsFromContent(t, content)
	}()

	select {
	case diags := <-done:
		elapsed := time.Since(start)
		t.Logf("completed in %s with %d diagnostics", elapsed, len(diags))
		return elapsed
	case r := <-panicked:
		t.Errorf("diagnostics collection panicked: %v", r)
		return time.Since(start)
	case <-time.After(ruleSweepTimeout):
		t.Fatalf("diagnostics collection did not complete within %s", ruleSweepTimeout)
		return 0
	}
}

func buildRuleSweepContent(source string, overrideEffectPackageJSON string) string {
	var sb strings.Builder
	sb.WriteString("// @filename: tsconfig.json\n")
	sb.WriteString(ruleSweepPluginConfig)
	if overrideEffectPackageJSON != "" {
		sb.WriteString("\n\n// @filename: /node_modules/effect/package.json\n")
		sb.WriteString(overrideEffectPackageJSON)
	}
	sb.WriteString("\n\n// @filename: test.ts\n")
	sb.WriteString(source)
	return sb.String()
}

// TestRuleSweepImportDefer is a fast regression test: `import.defer(...)` used
// to panic the checker via rules resolving the callee symbol or type
// (checkMetaProperty debug assertion). See the typeparser GetSymbolAtLocation and
// GetTypeAtLocation meta-property guards.
func TestRuleSweepImportDefer(t *testing.T) {
	t.Parallel()
	content := "// @filename: tsconfig.json\n" + ruleSweepPluginConfig +
		"\n\n// @filename: dep.ts\nexport {}\n" +
		"\n\n// @filename: test.ts\nexport {}\nimport.defer(\"./dep.js\")\n"
	runRuleSweepCase(t, content)
}

// TestRuleSweepTypeScriptCorpus sweeps the rules over typescript-go's own
// compiler/conformance test corpus, covering exotic syntax shapes the Effect
// fixtures never produce.
func TestRuleSweepTypeScriptCorpus(t *testing.T) {
	t.Parallel()
	if testing.Short() {
		t.Skip("corpus sweep is slow")
	}

	var files []string
	for _, dir := range []string{"compiler", "conformance"} {
		root := filepath.Join(bundledeffect.EffectTsGoRootPath(), "typescript-go", "testdata", "tests", "cases", dir)
		err := filepath.WalkDir(root, func(path string, d fs.DirEntry, err error) error {
			if err == nil && !d.IsDir() && (strings.HasSuffix(path, ".ts") || strings.HasSuffix(path, ".tsx")) {
				files = append(files, path)
			}
			return nil
		})
		if err != nil {
			t.Fatal(err)
		}
	}
	t.Logf("corpus size: %d files", len(files))

	for _, path := range files {
		raw, err := os.ReadFile(path)
		if err != nil {
			continue
		}
		name := filepath.Base(path)
		t.Run(name, func(t *testing.T) {
			t.Parallel()
			elapsed := runRuleSweepCase(t, buildRuleSweepContent(string(raw), ""))
			if elapsed > 5*time.Second {
				t.Logf("SLOW corpus file (%s): %s", elapsed, path)
			}
		})
	}
}

// TestRuleSweepEffectFixtures runs the sweep over this repo's own effect-v4
// rule fixtures (realistic Effect code). anyUnknownInErrorContext is off by
// default, so regular fixture runs never sweep these files with it.
func TestRuleSweepEffectFixtures(t *testing.T) {
	t.Parallel()
	if testing.Short() {
		t.Skip("fixture sweep is slow")
	}

	cases, err := DiscoverTestCases(bundledeffect.EffectV4)
	if err != nil {
		t.Fatal(err)
	}
	t.Logf("fixture count: %d", len(cases))

	for _, path := range cases {
		raw, err := os.ReadFile(path)
		if err != nil {
			continue
		}
		name := filepath.Base(path)
		t.Run(name, func(t *testing.T) {
			t.Parallel()
			elapsed := runRuleSweepCase(t, buildRuleSweepContent(string(raw), ""))
			if elapsed > 5*time.Second {
				t.Logf("SLOW fixture (%s): %s", elapsed, path)
			}
		})
	}
}

// TestRuleSweepEffectPackageSources pulls effect's own shipped src/*.ts files
// into the program as non-external, non-declaration files, so the sweep runs
// over the library's full source complexity.
func TestRuleSweepEffectPackageSources(t *testing.T) {
	t.Parallel()
	content := "// @filename: tsconfig.json\n" + ruleSweepPluginConfig +
		"\n\n// @filename: /node_modules/effect/src/__sweep__.ts\nexport * from \"./index.js\"\n" +
		"\n\n// @filename: test.ts\nimport * as E from \"/node_modules/effect/src/__sweep__.js\"\nexport const x = E\n"
	runRuleSweepCase(t, content)
}

// TestRuleSweepUnknownEffectVersion runs the sweep with effect version
// detection forced to Unknown (mangled package.json version), covering the
// v3/unknown property-iteration fallback in EffectType/LayerType.
func TestRuleSweepUnknownEffectVersion(t *testing.T) {
	t.Parallel()
	source := buildRuleSweepSource(20)

	t.Run("v4_detected", func(t *testing.T) {
		t.Parallel()
		runRuleSweepCase(t, buildRuleSweepContent(source, ""))
	})

	t.Run("version_unknown", func(t *testing.T) {
		t.Parallel()
		runRuleSweepCase(t, buildRuleSweepContent(source, unknownVersionEffectPackageJSON(t)))
	})
}

// unknownVersionEffectPackageJSON returns the bundled effect v4 package.json
// with its version mangled so that DetectEffectVersion returns Unknown.
func unknownVersionEffectPackageJSON(t *testing.T) string {
	t.Helper()
	raw, err := os.ReadFile(filepath.Join(bundledeffect.EffectTsGoRootPath(), "testdata", "tests", "effect-v4", "node_modules", "effect", "package.json"))
	if err != nil {
		t.Fatal(err)
	}
	patched := strings.Replace(string(raw), `"version": "4.`, `"version": "0.`, 1)
	if patched == string(raw) {
		t.Fatal("failed to patch effect package.json version")
	}
	return patched
}

// buildRuleSweepSource builds a file mixing recursive template-object types
// and Effect v4 usage, scaled by n repeated sections.
func buildRuleSweepSource(n int) string {
	var sb strings.Builder
	sb.WriteString(`import { Effect } from "effect"

interface KeyboardEvent { triggeredByAccelerator: boolean }
interface BrowserWindow { close(): void; isFocused(): boolean }
interface Menu { items: MenuItem[]; append(item: MenuItem): void; popup(): void; closePopup(): void }
interface MenuItem { label: string; enabled: boolean; visible: boolean; checked: boolean }
type Role = "about" | "quit" | "undo" | "redo" | "cut" | "copy" | "paste" | "toggleDevTools" | "reload" | "minimize" | "close" | "help" | "window" | "services" | "zoomIn" | "zoomOut" | "togglefullscreen"
interface MenuItemConstructorOptions {
	label?: string
	role?: Role
	type?: "normal" | "separator" | "submenu" | "checkbox" | "radio"
	accelerator?: string
	enabled?: boolean
	visible?: boolean
	checked?: boolean
	click?: (menuItem: MenuItem, browserWindow: BrowserWindow | undefined, event: KeyboardEvent) => void
	submenu?: MenuItemConstructorOptions[] | Menu
}
declare const MenuStatic: {
	buildFromTemplate(template: Array<MenuItemConstructorOptions | MenuItem>): Menu
	setApplicationMenu(menu: Menu | null): void
	getApplicationMenu(): Menu | null
}
`)
	for i := range n {
		fmt.Fprintf(&sb, `
const template%[1]d: MenuItemConstructorOptions[] = [
	{ label: "App%[1]d", submenu: [{ role: "about" }, { type: "separator" }, { role: "quit" }] },
	{ label: "Edit%[1]d", submenu: [{ role: "undo" }, { role: "redo" }, { type: "separator" }, { role: "cut" }, { role: "copy" }, { role: "paste" }] },
	{ label: "View%[1]d", submenu: [{ role: "reload" }, { role: "toggleDevTools" }, { label: "Custom%[1]d", click: (item, win, ev) => { if (win?.isFocused()) { win.close() } } }] },
	{ label: "Window%[1]d", submenu: [{ role: "minimize" }, { role: "close" }] },
]
export const menu%[1]d = MenuStatic.buildFromTemplate(template%[1]d)

export const program%[1]d = Effect.succeed(%[1]d).pipe(
	Effect.map((v) => v + 1),
	Effect.flatMap((v) => v > 0 ? Effect.succeed(v) : Effect.fail(new Error("neg"))),
	Effect.map((v) => v * 2),
	Effect.catchAll(() => Effect.succeed(0)),
)

export const gen%[1]d = Effect.gen(function*() {
	const a = yield* program%[1]d
	const m = MenuStatic.getApplicationMenu()
	if (m) { m.append({ label: "x", enabled: true, visible: true, checked: false }) }
	return a + template%[1]d.length
})
`, i)
	}
	return sb.String()
}
