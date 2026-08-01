package main

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"golang.org/x/mod/modfile"
)

func TestMergeExtraShimDeterministic(t *testing.T) {
	base := ExtraShim{
		ExtraFunctions: []string{"z", "a"},
		ExtraMethods: map[string][]string{
			"Checker": {"z", "a"},
		},
		ExtraFields: map[string][]string{
			"Node": {"end"},
		},
	}
	overlay := ExtraShim{
		ExtraFunctions: []string{"m", "a"},
		ExtraMethods: map[string][]string{
			"Checker": {"m", "a"},
			"Program": {"sourceFiles"},
		},
		ExtraFields: map[string][]string{
			"Node": {"pos", "end"},
		},
		IgnoreFunctions: []string{"internal", "internal"},
	}

	want := ExtraShim{
		ExtraFunctions: []string{"a", "m", "z"},
		ExtraMethods: map[string][]string{
			"Checker": {"a", "m", "z"},
			"Program": {"sourceFiles"},
		},
		ExtraFields: map[string][]string{
			"Node": {"end", "pos"},
		},
		IgnoreFunctions: []string{"internal"},
	}
	if got := mergeExtraShim(base, overlay); !reflect.DeepEqual(got, want) {
		t.Fatalf("mergeExtraShim() = %#v, want %#v", got, want)
	}
}

func TestUpdateGoWorkPreservesNonShimEntriesAndDropsStaleShimReplacements(t *testing.T) {
	repositoryRoot := t.TempDir()
	absoluteShim := filepath.ToSlash(filepath.Join(repositoryRoot, "shim", "absolute"))
	input := []byte(fmt.Sprintf(`go 1.26

use (
	.
	./etscore
	./shim/old
	./nested/../shim/normalized
	%s
	../shim
	./tsgolint
	./typescript-go
)

replace example.com/old-shim => ./shim/old

replace example.com/all-shims => ./shim

replace example.com/absolute-shim => %s

replace example.com/normalized-shim => ./nested/../shim/replacement

replace example.com/sibling-shim => ../shim

replace github.com/microsoft/typescript-go/shim/checker => ../external-checker

replace github.com/microsoft/typescript-go/shim => ../external-shims

replace example.com/unrelated => ../unrelated
`, absoluteShim, absoluteShim))

	got, err := updateGoWork(repositoryRoot, input, []string{"ls/change", "api"})
	if err != nil {
		t.Fatal(err)
	}
	workFile, err := modfile.ParseWork("go.work", got, nil)
	if err != nil {
		t.Fatal(err)
	}
	uses := make([]string, len(workFile.Use))
	for index, use := range workFile.Use {
		uses[index] = use.Path
	}
	wantUses := []string{".", "./etscore", "../shim", "./tsgolint", "./typescript-go", "./shim/api", "./shim/ls/change"}
	if !reflect.DeepEqual(uses, wantUses) {
		t.Fatalf("go.work uses = %v, want %v", uses, wantUses)
	}
	if len(workFile.Replace) != 2 || workFile.Replace[0].Old.Path != "example.com/sibling-shim" || workFile.Replace[0].New.Path != "../shim" || workFile.Replace[1].Old.Path != "example.com/unrelated" || workFile.Replace[1].New.Path != "../unrelated" {
		t.Fatalf("go.work replacements = %#v, want sibling and unrelated replacements", workFile.Replace)
	}
}

func TestLoadShimInputsRejectsUnknownOverlayPackage(t *testing.T) {
	base := t.TempDir()
	overlay := t.TempDir()
	writeTestFile(t, filepath.Join(overlay, "unknown", "extra-shim.json"), []byte(`{"ExtraFunctions":["f"]}`))

	_, err := loadShimInputs(base, []string{overlay})
	if err == nil || !strings.Contains(err.Error(), "unknown package path") {
		t.Fatalf("loadShimInputs() error = %v, want unknown package path", err)
	}
}

func TestLoadShimInputsPreservesCanonicalBaseOrder(t *testing.T) {
	base := t.TempDir()
	writeTestFile(t, filepath.Join(base, "checker", "extra-shim.json"), []byte(`{"ExtraFunctions":["z","a"]}`))

	inputs, err := loadShimInputs(base, nil)
	if err != nil {
		t.Fatal(err)
	}
	want := []string{"z", "a"}
	if got := inputs.extra["checker"].ExtraFunctions; !reflect.DeepEqual(got, want) {
		t.Fatalf("base ExtraFunctions = %v, want %v", got, want)
	}
}

func TestLoadShimInputsRejectsHelperConflict(t *testing.T) {
	base := t.TempDir()
	overlay := t.TempDir()
	writeTestFile(t, filepath.Join(base, "checker", "helper.go"), []byte("package checker\n\nconst value = 1\n"))
	writeTestFile(t, filepath.Join(overlay, "checker", "helper.go"), []byte("package checker\n\nconst value = 2\n"))

	_, err := loadShimInputs(base, []string{overlay})
	if err == nil || !strings.Contains(err.Error(), "conflicting handwritten shim helper") {
		t.Fatalf("loadShimInputs() error = %v, want helper conflict", err)
	}
}

func TestLoadShimInputsCopiesOverlayHelpersAndIgnoresGeneratedFiles(t *testing.T) {
	base := t.TempDir()
	overlay := t.TempDir()
	helper := []byte("package checker\n\nconst overlayHelper = true\n")
	writeTestFile(t, filepath.Join(overlay, "checker", "helper.go"), helper)
	writeTestFile(t, filepath.Join(overlay, "checker", "shim.go"), []byte("package checker\n"))
	writeTestFile(t, filepath.Join(overlay, "checker", "go.mod"), []byte("ignored"))
	writeTestFile(t, filepath.Join(overlay, "checker", "go.sum"), []byte("ignored"))

	inputs, err := loadShimInputs(base, []string{overlay})
	if err != nil {
		t.Fatal(err)
	}
	want := map[string][]byte{filepath.Join("checker", "helper.go"): helper}
	if !reflect.DeepEqual(inputs.helpers, want) {
		t.Fatalf("overlay helpers = %#v, want %#v", inputs.helpers, want)
	}
	output := t.TempDir()
	if err := writeShimHelpers(output, inputs.helpers); err != nil {
		t.Fatal(err)
	}
	if data, err := os.ReadFile(filepath.Join(output, "checker", "helper.go")); err != nil || !reflect.DeepEqual(data, helper) {
		t.Fatalf("copied overlay helper = %q, err %v", data, err)
	}
	for _, ignored := range []string{"shim.go", "go.mod", "go.sum"} {
		if _, err := os.Stat(filepath.Join(output, "checker", ignored)); !errors.Is(err, os.ErrNotExist) {
			t.Fatalf("ignored overlay file %s was copied: %v", ignored, err)
		}
	}
}

func TestResetOutputRootRemovesStaleHelpers(t *testing.T) {
	root := t.TempDir()
	writeTestFile(t, filepath.Join(root, "checker", "stale.go"), []byte("stale"))

	if err := resetOutputRoot(root); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(root, "checker", "stale.go")); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("stale helper still exists: %v", err)
	}
	if info, err := os.Stat(root); err != nil || !info.IsDir() {
		t.Fatalf("output root was not recreated: info=%v err=%v", info, err)
	}
}

func writeTestFile(t *testing.T, path string, data []byte) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, data, 0o644); err != nil {
		t.Fatal(err)
	}
}
