package main

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"strings"
	"sync"
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

	got, err := updateGoWork(
		repositoryRoot,
		filepath.Join(repositoryRoot, "typescript-go"),
		"github.com/microsoft/typescript-go",
		"github.com/microsoft/typescript-go/shim",
		input,
		[]string{"ls/change", "api"},
	)
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
	wantUses := []string{
		".",
		"./etscore",
		"../shim",
		"./tsgolint",
		"./shim/api",
		"./shim/_backport/api",
		"./shim/ls/change",
		"./shim/_backport/ls/change",
		"./typescript-go",
	}
	if !reflect.DeepEqual(uses, wantUses) {
		t.Fatalf("go.work uses = %v, want %v", uses, wantUses)
	}
	if len(workFile.Replace) != 7 ||
		workFile.Replace[0].Old.Path != "example.com/sibling-shim" || workFile.Replace[0].New.Path != "../shim" ||
		workFile.Replace[1].Old.Path != "example.com/unrelated" || workFile.Replace[1].New.Path != "../unrelated" ||
		workFile.Replace[2].Old.Path != "github.com/microsoft/typescript-go" ||
		workFile.Replace[2].Old.Version != "v0.0.0" || workFile.Replace[2].New.Path != "./typescript-go" ||
		workFile.Replace[3].Old.Path != "github.com/microsoft/typescript-go/shim/api" ||
		workFile.Replace[3].New.Path != "./shim/api" ||
		workFile.Replace[4].Old.Path != "github.com/microsoft/TypeScript/tsc/shim/api" ||
		workFile.Replace[4].New.Path != "./shim/_backport/api" ||
		workFile.Replace[5].Old.Path != "github.com/microsoft/typescript-go/shim/ls/change" ||
		workFile.Replace[5].New.Path != "./shim/ls/change" ||
		workFile.Replace[6].Old.Path != "github.com/microsoft/TypeScript/tsc/shim/ls/change" ||
		workFile.Replace[6].New.Path != "./shim/_backport/ls/change" {
		t.Fatalf("go.work replacements = %#v, want preserved, provider, and backport replacements", workFile.Replace)
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
	writeTestFile(t, filepath.Join(overlay, "checker", "helper.go"), bytes.ReplaceAll(helper, []byte("\n"), []byte("\r\n")))
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

func TestPreparePackageLoadUsesStablePaths(t *testing.T) {
	repositoryRoot := t.TempDir()
	typescriptGoRoot := filepath.Join(repositoryRoot, "typescript-go")
	writeTestFile(t, filepath.Join(typescriptGoRoot, "go.mod"), []byte("module github.com/microsoft/typescript-go\n\ngo 1.26\n"))
	writeTestFile(t, filepath.Join(typescriptGoRoot, "go.sum"), nil)

	modFiles := make([]string, 8)
	errors := make([]error, len(modFiles))
	start := make(chan struct{})
	var group sync.WaitGroup
	for index := range modFiles {
		group.Add(1)
		go func() {
			defer group.Done()
			<-start
			_, modFiles[index], errors[index] = preparePackageLoad(repositoryRoot, typescriptGoRoot)
		}()
	}
	close(start)
	group.Wait()
	for index, err := range errors {
		if err != nil {
			t.Fatalf("preparePackageLoad call %d failed: %v", index, err)
		}
		if modFiles[index] != modFiles[0] {
			t.Fatalf("modfile paths differ between runs: %q and %q", modFiles[0], modFiles[index])
		}
	}
	if matched, err := filepath.Match(filepath.Join(repositoryRoot, ".tmp", "gen-shims-load-*", "typescript-go.mod"), modFiles[0]); err != nil || !matched {
		t.Fatalf("modfile path %q is not content-addressed: matched=%v err=%v", modFiles[0], matched, err)
	}
	loadRoot := filepath.Dir(modFiles[0])
	for _, relative := range []string{".complete", "typescript-go.mod", "typescript-go.sum", filepath.Join("etscore", "go.mod"), filepath.Join("etscore", "etscore.go")} {
		if _, err := os.Stat(filepath.Join(loadRoot, relative)); err != nil {
			t.Fatalf("stable load file %s does not exist: %v", relative, err)
		}
	}
}

func TestRewriteImportPrefixPreservesLinknameTargets(t *testing.T) {
	source := []byte(`package ast

import "github.com/microsoft/TypeScript/tsc/internal/ast"
import _ "unsafe"

type Node = ast.Node

//go:linkname CanHaveDecorators github.com/microsoft/TypeScript/tsc/internal/ast.CanHaveDecorators
func CanHaveDecorators(node *ast.Node) bool
`)

	got, err := rewriteImportPrefix(
		source,
		"github.com/microsoft/TypeScript/tsc/internal/",
		"github.com/microsoft/TypeScript/tsc/shim/",
	)
	if err != nil {
		t.Fatal(err)
	}
	text := string(got)
	if !strings.Contains(text, `"github.com/microsoft/TypeScript/tsc/shim/ast"`) {
		t.Fatalf("rewritten source does not import provider shim:\n%s", text)
	}
	if !strings.Contains(text, "//go:linkname CanHaveDecorators github.com/microsoft/TypeScript/tsc/internal/ast.CanHaveDecorators") {
		t.Fatalf("rewritten source changed linkname target:\n%s", text)
	}
}

func TestGenerateBackportReexportsProviderShims(t *testing.T) {
	providerRoot := filepath.Join(t.TempDir(), "shim")
	backportRoot := filepath.Join(providerRoot, "_backport")
	writeTestFile(t, filepath.Join(providerRoot, "ast", "shim.go"), []byte(`package ast

import "github.com/microsoft/typescript-go/internal/ast"
import _ "unsafe"

type Node = ast.Node

//go:linkname CanHaveDecorators github.com/microsoft/typescript-go/internal/ast.CanHaveDecorators
func CanHaveDecorators(node *ast.Node) bool
`))

	if err := generateBackport(
		providerRoot,
		backportRoot,
		"github.com/microsoft/typescript-go/internal/",
		"github.com/microsoft/typescript-go/shim",
	); err != nil {
		t.Fatal(err)
	}
	shim, err := os.ReadFile(filepath.Join(backportRoot, "ast", "shim.go"))
	if err != nil {
		t.Fatal(err)
	}
	text := string(shim)
	if !strings.Contains(text, `"github.com/microsoft/typescript-go/shim/ast"`) {
		t.Fatalf("backport does not import the provider shim:\n%s", text)
	}
	if !strings.Contains(text, "//go:linkname CanHaveDecorators github.com/microsoft/typescript-go/internal/ast.CanHaveDecorators") {
		t.Fatalf("backport changed the linkname target:\n%s", text)
	}
	goMod, err := os.ReadFile(filepath.Join(backportRoot, "ast", "go.mod"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(goMod), "module github.com/microsoft/TypeScript/tsc/shim/ast") ||
		!strings.Contains(string(goMod), "require github.com/microsoft/typescript-go/shim/ast v0.0.0") {
		t.Fatalf("unexpected backport go.mod:\n%s", goMod)
	}
	if _, err := os.Stat(filepath.Join(backportRoot, "_backport")); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("backport recursively copied itself: %v", err)
	}
}

func TestUpdateGoWorkUsesProviderModulesDirectlyForModernCompiler(t *testing.T) {
	repositoryRoot := t.TempDir()
	input := []byte(`go 1.26

use (
	.
	./etscore
	./typescript-go
	./shim/stale
)
`)

	got, err := updateGoWork(
		repositoryRoot,
		filepath.Join(repositoryRoot, "typescript", "tsc"),
		"github.com/microsoft/TypeScript/tsc",
		"github.com/microsoft/TypeScript/tsc/shim",
		input,
		[]string{"ast"},
	)
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
	want := []string{".", "./etscore", "./shim/ast", "./typescript/tsc"}
	if !reflect.DeepEqual(uses, want) {
		t.Fatalf("go.work uses = %v, want %v", uses, want)
	}
	if len(workFile.Replace) != 2 ||
		workFile.Replace[0].Old.Path != "github.com/microsoft/TypeScript/tsc" ||
		workFile.Replace[0].Old.Version != "v0.0.0" || workFile.Replace[0].New.Path != "./typescript/tsc" ||
		workFile.Replace[1].Old.Path != "github.com/microsoft/TypeScript/tsc/shim/ast" ||
		workFile.Replace[1].Old.Version != "v0.0.0" || workFile.Replace[1].New.Path != "./shim/ast" {
		t.Fatalf("go.work replacements = %#v, want compiler and provider shim replacements", workFile.Replace)
	}
}

func TestGitInputDigestTracksRelevantRepositoryState(t *testing.T) {
	repositoryRoot := t.TempDir()
	runGitTest(t, repositoryRoot, "init")
	runGitTest(t, repositoryRoot, "config", "user.email", "test@example.com")
	runGitTest(t, repositoryRoot, "config", "user.name", "Test")
	writeTestFile(t, filepath.Join(repositoryRoot, "main.go"), []byte("package main\n"))
	writeTestFile(t, filepath.Join(repositoryRoot, "config", "extra-shim.json"), []byte("{}\n"))
	writeTestFile(t, filepath.Join(repositoryRoot, ".gitignore"), []byte("config/ignored.go\n"))
	writeTestFile(t, filepath.Join(repositoryRoot, "unrelated.txt"), []byte("initial\n"))
	runGitTest(t, repositoryRoot, "add", ".")
	runGitTest(t, repositoryRoot, "commit", "-m", "initial")

	inputs := []gitInput{{root: repositoryRoot, paths: []string{"main.go", "config"}}}
	initial, err := gitInputDigest(inputs)
	if err != nil {
		t.Fatal(err)
	}

	writeTestFile(t, filepath.Join(repositoryRoot, "unrelated.txt"), []byte("changed\n"))
	unrelated, err := gitInputDigest(inputs)
	if err != nil {
		t.Fatal(err)
	}
	if unrelated != initial {
		t.Fatalf("unrelated change altered digest: %s != %s", unrelated, initial)
	}

	writeTestFile(t, filepath.Join(repositoryRoot, "main.go"), []byte("package main\n\nconst changed = true\n"))
	tracked, err := gitInputDigest(inputs)
	if err != nil {
		t.Fatal(err)
	}
	if tracked == initial {
		t.Fatal("tracked input change did not alter digest")
	}

	writeTestFile(t, filepath.Join(repositoryRoot, "config", "helper.go"), []byte("package config\n"))
	untracked, err := gitInputDigest(inputs)
	if err != nil {
		t.Fatal(err)
	}
	if untracked == tracked {
		t.Fatal("untracked input change did not alter digest")
	}

	writeTestFile(t, filepath.Join(repositoryRoot, "config", "ignored.go"), []byte("package config\n"))
	ignored, err := gitInputDigest(inputs)
	if err != nil {
		t.Fatal(err)
	}
	if ignored == untracked {
		t.Fatal("ignored input change did not alter digest")
	}
}

func TestShimCacheStoresAndRestoresCompleteOutput(t *testing.T) {
	cacheRoot := filepath.Join(t.TempDir(), "cache")
	outputRoot := filepath.Join(t.TempDir(), "shim")
	writeTestFile(t, filepath.Join(outputRoot, generatedMarker), []byte("generated\n"))
	writeTestFile(t, filepath.Join(outputRoot, "checker", "shim.go"), []byte("package checker\n"))

	if err := storeShimCache(cacheRoot, "digest", outputRoot); err != nil {
		t.Fatal(err)
	}
	writeTestFile(t, filepath.Join(outputRoot, "checker", "shim.go"), []byte("corrupted\n"))
	writeTestFile(t, filepath.Join(outputRoot, "stale.go"), []byte("stale\n"))

	hit, err := restoreShimCache(cacheRoot, "digest", outputRoot)
	if err != nil {
		t.Fatal(err)
	}
	if !hit {
		t.Fatal("restoreShimCache() missed stored entry")
	}
	if data, err := os.ReadFile(filepath.Join(outputRoot, "checker", "shim.go")); err != nil || string(data) != "package checker\n" {
		t.Fatalf("restored shim = %q, err %v", data, err)
	}
	if _, err := os.Stat(filepath.Join(outputRoot, "stale.go")); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("stale output survived restore: %v", err)
	}
}

func TestShimCacheIgnoresIncompleteEntry(t *testing.T) {
	cacheRoot := t.TempDir()
	outputRoot := filepath.Join(t.TempDir(), "shim")
	writeTestFile(t, filepath.Join(cacheRoot, shimCacheVersion, "digest", "shim", "checker", "shim.go"), []byte("package checker\n"))

	hit, err := restoreShimCache(cacheRoot, "digest", outputRoot)
	if err != nil {
		t.Fatal(err)
	}
	if hit {
		t.Fatal("restoreShimCache() hit incomplete entry")
	}
}

func runGitTest(t *testing.T, directory string, arguments ...string) {
	t.Helper()
	command := exec.Command("git", append([]string{"-C", directory}, arguments...)...)
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("git %s failed: %v\n%s", strings.Join(arguments, " "), err, output)
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
