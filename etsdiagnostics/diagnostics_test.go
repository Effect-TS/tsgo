package etsdiagnostics

import (
	"bytes"
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"regexp"
	"strings"
	"testing"
	"testing/fstest"

	_ "github.com/effect-ts/tsgo/etscheckerhooks"
	"github.com/effect-ts/tsgo/internal/bundledeffect"
)

func TestParseSeverityFilter(t *testing.T) {
	t.Parallel()

	filter := parseSeverityFilter(" ERROR, warning,invalid ")
	if _, ok := filter[severityError]; !ok {
		t.Fatal("expected error severity")
	}
	if _, ok := filter[severityWarning]; !ok {
		t.Fatal("expected warning severity")
	}
	if _, ok := filter[severityMessage]; ok {
		t.Fatal("did not expect message severity")
	}
	if filter := parseSeverityFilter("invalid"); filter != nil {
		t.Fatal("an entirely invalid filter should disable filtering")
	}
}

func TestParseLSPConfigPresence(t *testing.T) {
	t.Parallel()

	if config, provided, err := parseLSPConfig(nil); err != nil || provided || config != nil {
		t.Fatalf("unexpected omitted config result: config=%#v provided=%t err=%v", config, provided, err)
	}
	empty := ""
	if _, provided, err := parseLSPConfig(&empty); err == nil || !provided {
		t.Fatalf("expected an explicitly empty config to fail: provided=%t err=%v", provided, err)
	}
	null := "null"
	if config, provided, err := parseLSPConfig(&null); err != nil || !provided || config == nil {
		t.Fatalf("unexpected null config result: config=%#v provided=%t err=%v", config, provided, err)
	}
}

func TestWriteJSONOutput(t *testing.T) {
	t.Parallel()

	diagnostics := []formattedDiagnostic{{
		File: "/workspace/main.ts", Start: 2, Length: 4, Line: 1, Column: 3,
		EndLine: 1, EndColumn: 7, Severity: severityError, Code: 377001,
		Name: "floatingEffect", Message: "Effect must be handled",
	}}
	wantSummary := summary{FilesChecked: 1, TotalFiles: 1, Errors: 1}
	var output bytes.Buffer
	if err := writeOutput(&output, "json", diagnostics, nil, wantSummary); err != nil {
		t.Fatal(err)
	}
	for _, expected := range []string{
		`"file": "/workspace/main.ts"`,
		`"severity": "error"`,
		`"name": "floatingEffect"`,
		`"filesChecked": 1`,
	} {
		if !strings.Contains(output.String(), expected) {
			t.Fatalf("output did not contain %q:\n%s", expected, output.String())
		}
	}
}

func TestWriteGitHubActionsOutput(t *testing.T) {
	t.Parallel()

	diagnostics := []formattedDiagnostic{{
		File: "/workspace/main.ts", Line: 2, Column: 3, EndLine: 2, EndColumn: 8,
		Severity: severityMessage, Name: "floatingEffect", Message: "first%\nsecond",
	}}
	var output bytes.Buffer
	if err := writeOutput(&output, "github-actions", diagnostics, nil, summary{FilesChecked: 1, TotalFiles: 1, Messages: 1}); err != nil {
		t.Fatal(err)
	}
	want := "::notice file=/workspace/main.ts,line=2,col=3,endLine=2,endColumn=8,title=floatingEffect::first%25%0Asecond\n"
	if !strings.HasPrefix(output.String(), want) {
		t.Fatalf("unexpected output:\n%s", output.String())
	}
}

func TestWriteFilesBlock(t *testing.T) {
	t.Parallel()

	files := []fileEffectVersion{
		{File: "/workspace/main.ts", DetectedEffect: "unknown", SupportedEffect: "v3"},
	}
	var output bytes.Buffer
	if err := writeOutput(&output, "text", nil, files, summary{TotalFiles: 1, FilesChecked: 1}); err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(output.String(), "/workspace/main.ts: detected=unknown, supported=v3") {
		t.Fatalf("output missing per-file version block:\n%s", output.String())
	}
}

func runRequest(t *testing.T, req request) string {
	t.Helper()
	encoded, err := json.Marshal(req)
	if err != nil {
		t.Fatal(err)
	}
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	if status := Run(context.Background(), []string{string(encoded)}, &stdout, &stderr); status != 1 {
		t.Fatalf("unexpected status %d; stderr:\n%s", status, stderr.String())
	}
	return stdout.String()
}

func runNativeDiagnosticsJSON(t *testing.T, req request) jsonOutput {
	t.Helper()
	cwd, err := filepath.Abs("testdata/native-diagnostics")
	if err != nil {
		t.Fatal(err)
	}
	req.CWD = cwd
	req.Format = "json"
	stdout := runRequest(t, req)
	var output jsonOutput
	if err := json.Unmarshal([]byte(stdout), &output); err != nil {
		t.Fatalf("invalid JSON output: %v\n%s", err, stdout)
	}
	return output
}

func TestRunJSON(t *testing.T) {
	t.Parallel()

	for name, req := range map[string]request{
		"project": {Project: "tsconfig.json"},
		"file":    {File: "main.ts"},
	} {
		t.Run(name, func(t *testing.T) {
			t.Parallel()
			output := runNativeDiagnosticsJSON(t, req)
			if len(output.Diagnostics) != 1 {
				t.Fatalf("expected one diagnostic, got %d", len(output.Diagnostics))
			}
			diagnostic := output.Diagnostics[0]
			if diagnostic.Name != "asyncFunction" || diagnostic.Code != 377081 || diagnostic.Severity != severityError {
				t.Fatalf("unexpected diagnostic: %#v", diagnostic)
			}
			if strings.Contains(diagnostic.Message, "effect(asyncFunction)") {
				t.Fatalf("message includes redundant rule name: %q", diagnostic.Message)
			}
			if output.Files != nil {
				t.Fatalf("expected no files list without listFiles: %#v", output.Files)
			}
		})
	}
}

func TestRunProjectJSONListFiles(t *testing.T) {
	t.Parallel()

	output := runNativeDiagnosticsJSON(t, request{Project: "tsconfig.json", ListFiles: true})
	if len(output.Files) == 0 {
		t.Fatalf("expected at least one file entry: %#v", output)
	}
	for _, entry := range output.Files {
		if entry.File == "" {
			t.Fatalf("empty file name in entry: %#v", entry)
		}
		if entry.SupportedEffect != "v3" && entry.SupportedEffect != "v4" {
			t.Fatalf("unexpected supported version %q for %s", entry.SupportedEffect, entry.File)
		}
		if entry.DetectedEffect == "" {
			t.Fatalf("empty detected version for %s", entry.File)
		}
	}
}

const missingEffectContextSource = `import { Context, Effect, Layer } from "effect"

class Db extends Context.Service<Db>()("Db", { make: Effect.succeed({}) }) {
  static Default = Layer.effect(this, this.make)
}

class Fs extends Context.Service<Fs>()("Fs", { make: Effect.succeed({}) }) {
  static Default = Layer.effect(this, this.make)
}

class Repo extends Context.Service<Repo>()("Repo", { make: Effect.as(Effect.andThen(Db, Fs), {}) }) {
  static Default = Layer.effect(this, this.make)
}

const live = Repo.Default.pipe(Layer.provide(Fs.Default))

Effect.gen(function*() {
  yield* Fs
  yield* Db
}).pipe(
  Effect.provide(live),
  Effect.runPromise
)
`

func writeEffectV4Project(t *testing.T, source string) string {
	t.Helper()
	files := make(map[string]any)
	if err := bundledeffect.MountEffect(bundledeffect.EffectV4, files); err != nil {
		t.Skip("Effect not installed:", err)
	}
	root := t.TempDir()
	files["/.src/tsconfig.json"] = &fstest.MapFile{Data: []byte(`{
  "compilerOptions": {
    "strict": true,
    "skipLibCheck": true,
    "plugins": [{ "name": "@effect/language-service", "diagnosticSeverity": { "discardedRunPromiseToRunFork": "off" } }]
  },
  "files": ["main.ts"]
}`)}
	files["/.src/main.ts"] = &fstest.MapFile{Data: []byte(source)}
	for name, file := range files {
		path := filepath.Join(root, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, file.(*fstest.MapFile).Data, 0o644); err != nil {
			t.Fatal(err)
		}
	}
	return filepath.Join(root, ".src")
}

func runMissingEffectContext(t *testing.T, format string) (string, string) {
	t.Helper()
	cwd := writeEffectV4Project(t, missingEffectContextSource)
	stdout := runRequest(t, request{CWD: cwd, Project: "tsconfig.json", Format: format})
	return stdout, filepath.Join(cwd, "main.ts")
}

func TestRunJSONReportsRelatedInformation(t *testing.T) {
	t.Parallel()

	stdout, file := runMissingEffectContext(t, "json")
	var output struct {
		Diagnostics []struct {
			Code               int32            `json:"code"`
			Message            string           `json:"message"`
			RelatedInformation []map[string]any `json:"relatedInformation"`
		} `json:"diagnostics"`
	}
	if err := json.Unmarshal([]byte(stdout), &output); err != nil {
		t.Fatalf("invalid JSON output: %v\n%s", err, stdout)
	}
	if len(output.Diagnostics) != 1 || output.Diagnostics[0].Code != 377004 {
		t.Fatalf("expected one missingEffectContext diagnostic:\n%s", stdout)
	}
	want := []map[string]any{
		{
			"file": file, "start": float64(580), "length": float64(14),
			"line": float64(21), "column": float64(3), "endLine": float64(21), "endColumn": float64(17),
			"code": float64(377059), "message": "Adjusting this layer composition could provide the missing service.",
		},
		{
			"file": file, "start": float64(264), "length": float64(7),
			"line": float64(8), "column": float64(10), "endLine": float64(8), "endColumn": float64(17),
			"code": float64(377060), "message": "This layer provides the missing service Fs.",
		},
	}
	if got := output.Diagnostics[0].RelatedInformation; !reflect.DeepEqual(got, want) {
		t.Fatalf("unexpected relatedInformation\ngot:  %#v\nwant: %#v", got, want)
	}
}

func TestRunPrettyReportsRelatedInformation(t *testing.T) {
	t.Parallel()

	stdout, file := runMissingEffectContext(t, "pretty")
	plain := regexp.MustCompile("\x1b\\[[0-9;]*m").ReplaceAllString(stdout, "")
	want := file + ":22:3 - error effect(missingEffectContext): This Effect requires a service that is missing from the expected Effect context: `Db | Fs`.\n" +
		"\n" +
		"22   Effect.runPromise\n" +
		"     ~~~~~~~~~~~~~~~~~\n" +
		"\n" +
		"  " + file + ":21:3 - Adjusting this layer composition could provide the missing service.\n" +
		"    21   Effect.provide(live),\n" +
		"         ~~~~~~~~~~~~~~\n" +
		"\n" +
		"  " + file + ":8:10 - This layer provides the missing service Fs.\n" +
		"    8   static Default = Layer.effect(this, this.make)\n" +
		"               ~~~~~~~\n" +
		"\n" +
		"Checked 1 files out of 1 files. \n"
	if !strings.HasPrefix(plain, want) {
		t.Fatalf("unexpected pretty output\ngot:\n%s\nwant prefix:\n%s", plain, want)
	}
}
