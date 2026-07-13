package etsdiagnostics

import (
	"bytes"
	"context"
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"

	_ "github.com/effect-ts/tsgo/etscheckerhooks"
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
	if err := writeOutput(&output, "json", diagnostics, wantSummary); err != nil {
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
	if err := writeOutput(&output, "github-actions", diagnostics, summary{FilesChecked: 1, TotalFiles: 1, Messages: 1}); err != nil {
		t.Fatal(err)
	}
	want := "::notice file=/workspace/main.ts,line=2,col=3,endLine=2,endColumn=8,title=floatingEffect::first%25%0Asecond\n"
	if !strings.HasPrefix(output.String(), want) {
		t.Fatalf("unexpected output:\n%s", output.String())
	}
}

func TestRunProjectJSON(t *testing.T) {
	t.Parallel()

	cwd, err := filepath.Abs("testdata/native-diagnostics")
	if err != nil {
		t.Fatal(err)
	}
	request, err := json.Marshal(request{
		CWD: cwd, Project: "tsconfig.json", Format: "json",
	})
	if err != nil {
		t.Fatal(err)
	}
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	if status := Run(context.Background(), []string{string(request)}, &stdout, &stderr); status != 1 {
		t.Fatalf("unexpected status %d; stderr:\n%s", status, stderr.String())
	}
	var output jsonOutput
	if err := json.Unmarshal(stdout.Bytes(), &output); err != nil {
		t.Fatalf("invalid JSON output: %v\n%s", err, stdout.String())
	}
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
}
