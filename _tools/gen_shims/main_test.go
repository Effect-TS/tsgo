package main

import (
	"bytes"
	"errors"
	"flag"
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

func TestGeneratedExtraStructPreservesPointerLayout(t *testing.T) {
	savedPackages, savedDependencies := packagesToShim, modernProviderDependencies
	savedInternalPrefix, savedShimPrefix := tsgoInternalPrefix, providerShimModulePrefix
	savedFlags, savedArgs := flag.CommandLine, os.Args
	t.Cleanup(func() {
		packagesToShim, modernProviderDependencies = savedPackages, savedDependencies
		tsgoInternalPrefix, providerShimModulePrefix = savedInternalPrefix, savedShimPrefix
		flag.CommandLine, os.Args = savedFlags, savedArgs
	})
	packagesToShim = []string{"fixture"}
	modernProviderDependencies = nil
	flag.CommandLine = flag.NewFlagSet(t.Name(), flag.ContinueOnError)

	root := t.TempDir()
	sourceRoot := filepath.Join(root, "typescript", "tsc")
	writeTestFile(t, filepath.Join(root, "go.work"), []byte("go 1.26\n"))
	writeTestFile(t, filepath.Join(sourceRoot, "go.mod"), []byte("module github.com/microsoft/TypeScript/tsc\n\ngo 1.26\n"))
	writeTestFile(t, filepath.Join(sourceRoot, "go.sum"), nil)
	writeTestFile(t, filepath.Join(sourceRoot, "internal", "fixture", "fixture.go"), []byte(`package fixture

type hidden struct { words [2]uint64 }
type Fixture struct {
	dependency *hidden
	value int64
	trailing [2]uint64
}
var Sample = Fixture{dependency: &hidden{}, value: 42}
`))
	writeTestFile(t, filepath.Join(root, "_tools", "gen_shims", "config", "fixture", "extra-shim.json"), []byte(`{"ExtraFields":{"Fixture":["value"]}}`))
	os.Args = []string{
		t.Name(),
		"-repository-root", root,
		"-source-root", sourceRoot,
		"-module-prefix", "github.com/microsoft/TypeScript/tsc",
		"-provider-shim-prefix", canonicalShimModulePrefix,
		"-no-cache",
	}
	if err := run(); err != nil {
		t.Fatalf("generate fixture shim: %v", err)
	}

	shimRoot := filepath.Join(root, "shim", "fixture")
	writeTestFile(t, filepath.Join(shimRoot, "layout_test.go"), []byte(`package fixture

import (
	"reflect"
	"testing"
)

func TestPointerFieldLayout(t *testing.T) {
	mirror := reflect.TypeOf(extra_Fixture{})
	dependency, ok := mirror.FieldByName("dependency")
	if !ok {
		t.Fatal("generated mirror is missing dependency")
	}
	if got := dependency.Type.Kind(); got != reflect.Pointer {
		t.Errorf("dependency kind = %v, want ptr", got)
	}
	originalValue, ok := reflect.TypeOf(Sample).FieldByName("value")
	if !ok {
		t.Fatal("fixture is missing value")
	}
	mirroredValue, ok := mirror.FieldByName("value")
	if !ok {
		t.Fatal("generated mirror is missing value")
	}
	if got, want := mirroredValue.Offset, originalValue.Offset; got != want {
		t.Errorf("value offset = %d, want %d", got, want)
	}
	if got := Fixture_value(&Sample); got != 42 {
		t.Errorf("Fixture_value() = %d, want 42", got)
	}
}
`))
	command := exec.Command("go", "test", "-count=1", "-v", ".")
	command.Dir = shimRoot
	command.Env = append(os.Environ(), "GOWORK="+filepath.Join(root, "go.work"))
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("compile and exercise generated pointer layout: %v\n%s", err, output)
	}
}

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
	assertGoWork(t, got, []string{
		".",
		"./etscore",
		"../shim",
		"./tsgolint",
		"./shim/api",
		"./shim/_backport/api",
		"./shim/ls/change",
		"./shim/_backport/ls/change",
		"./typescript-go",
	}, []string{
		"example.com/sibling-shim => ../shim",
		"example.com/unrelated => ../unrelated",
		"github.com/microsoft/typescript-go v0.0.0 => ./typescript-go",
		"github.com/microsoft/typescript-go/shim/api v0.0.0 => ./shim/api",
		"github.com/microsoft/TypeScript/tsc/shim/api v0.0.0 => ./shim/_backport/api",
		"github.com/microsoft/typescript-go/shim/ls/change v0.0.0 => ./shim/ls/change",
		"github.com/microsoft/TypeScript/tsc/shim/ls/change v0.0.0 => ./shim/_backport/ls/change",
	})
}

func assertGoWork(t *testing.T, data []byte, wantUses, wantReplaces []string) {
	t.Helper()
	workFile, err := modfile.ParseWork("go.work", data, nil)
	if err != nil {
		t.Fatal(err)
	}
	uses := make([]string, len(workFile.Use))
	for index, use := range workFile.Use {
		uses[index] = use.Path
	}
	if !reflect.DeepEqual(uses, wantUses) {
		t.Errorf("go.work uses = %v, want %v", uses, wantUses)
	}
	replaces := make([]string, len(workFile.Replace))
	for index, replace := range workFile.Replace {
		replaces[index] = strings.TrimSpace(replace.Old.Path+" "+replace.Old.Version) + " => " + replace.New.Path
	}
	if !reflect.DeepEqual(replaces, wantReplaces) {
		t.Errorf("go.work replacements = %q, want %q", replaces, wantReplaces)
	}
}

func TestLoadShimInputsRejectsInvalidOverlay(t *testing.T) {
	for _, tc := range []struct {
		name    string
		base    map[string]string
		overlay map[string]string
		want    string
	}{
		{
			name:    "unknown package",
			overlay: map[string]string{"unknown/extra-shim.json": `{"ExtraFunctions":["f"]}`},
			want:    "unknown package path",
		},
		{
			name:    "helper conflict",
			base:    map[string]string{"checker/helper.go": "package checker\n\nconst value = 1\n"},
			overlay: map[string]string{"checker/helper.go": "package checker\n\nconst value = 2\n"},
			want:    "conflicting handwritten shim helper",
		},
	} {
		t.Run(tc.name, func(t *testing.T) {
			base := t.TempDir()
			overlay := t.TempDir()
			for root, files := range map[string]map[string]string{base: tc.base, overlay: tc.overlay} {
				for relative, content := range files {
					writeTestFile(t, filepath.Join(root, filepath.FromSlash(relative)), []byte(content))
				}
			}
			_, err := loadShimInputs(base, []string{overlay})
			if err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("loadShimInputs() error = %v, want %s", err, tc.want)
			}
		})
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

	adapters := map[string][]byte{"ast/compatibility.go": []byte(`package ast

import "github.com/microsoft/typescript-go/internal/ast"

func NewNode(name string) *ast.Node { return ast.NewNode(name, false) }
`)}

	if err := generateBackport(
		providerRoot,
		backportRoot,
		"github.com/microsoft/typescript-go/internal/",
		"github.com/microsoft/typescript-go/shim",
		adapters,
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
	compatibility, err := os.ReadFile(filepath.Join(backportRoot, "ast", "compatibility.go"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(compatibility), "ast.NewNode(name, false)") {
		t.Fatalf("backport must adapt calls to the original provider API:\n%s", compatibility)
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
	assertGoWork(t, got, []string{".", "./etscore", "./shim/ast", "./typescript/tsc"}, []string{
		"github.com/microsoft/TypeScript/tsc v0.0.0 => ./typescript/tsc",
		"github.com/microsoft/TypeScript/tsc/shim/ast v0.0.0 => ./shim/ast",
	})
}

func TestGitInputDigestTracksRelevantRepositoryState(t *testing.T) {
	repositoryRoot := t.TempDir()
	writeTestFile(t, filepath.Join(repositoryRoot, "main.go"), []byte("package main\n"))
	writeTestFile(t, filepath.Join(repositoryRoot, "config", "extra-shim.json"), []byte("{}\n"))
	writeTestFile(t, filepath.Join(repositoryRoot, ".gitignore"), []byte("config/ignored.go\n"))
	writeTestFile(t, filepath.Join(repositoryRoot, "unrelated.txt"), []byte("initial\n"))
	initGitTestRepo(t, repositoryRoot)

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

func TestShimInputDigestIncludesProviderOverlays(t *testing.T) {
	repositoryRoot := t.TempDir()
	writeTestFile(t, filepath.Join(repositoryRoot, "_tools", "gen_shims", "main.go"), []byte("package main\n"))
	writeTestFile(
		t,
		filepath.Join(repositoryRoot, "_tools", "gen_shims", "providers", "typescript", "checker", "integration.go"),
		[]byte("package checker\n"),
	)
	initGitTestRepo(t, repositoryRoot)

	sourceRoot := t.TempDir()
	writeTestFile(t, filepath.Join(sourceRoot, "go.mod"), []byte("module example\n"))
	initGitTestRepo(t, sourceRoot)

	digest := func() string {
		t.Helper()
		value, err := shimInputDigest(repositoryRoot, sourceRoot, "example", "example/shim", shimInputs{})
		if err != nil {
			t.Fatal(err)
		}
		return value
	}

	initial := digest()
	writeTestFile(
		t,
		filepath.Join(repositoryRoot, "_tools", "gen_shims", "providers", "typescript", "checker", "integration.go"),
		[]byte("package checker\n\nconst changed = true\n"),
	)
	durableChange := digest()
	if durableChange == initial {
		t.Fatal("provider overlay change did not alter digest")
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

func initGitTestRepo(t *testing.T, directory string) {
	t.Helper()
	runGitTest(t, directory, "init")
	runGitTest(t, directory, "config", "user.email", "test@example.com")
	runGitTest(t, directory, "config", "user.name", "Test")
	runGitTest(t, directory, "add", ".")
	runGitTest(t, directory, "commit", "-m", "initial")
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

func TestBackportAdaptsCanonicalAPIWithoutChangingProviderAPI(t *testing.T) {
	root := t.TempDir()
	providerRoot := filepath.Join(root, "provider")
	backportRoot := filepath.Join(root, "facade")
	writeTestFile(t, filepath.Join(root, "go.mod"), []byte("module example.com/facade\n\ngo 1.26\n"))
	writeTestFile(t, filepath.Join(root, "internal", "paths", "paths.go"), []byte(`package paths

type Options struct { Sensitive bool }
func NewPath(directory string, options Options) string {
    if options.Sensitive { return directory + ":sensitive" }
    return directory
}
func IsSensitive(options Options) bool { return options.Sensitive }
`))
	writeTestFile(t, filepath.Join(providerRoot, "paths", "shim.go"), []byte(`package paths

import raw "example.com/facade/internal/paths"
import _ "unsafe"

type Options = raw.Options
//go:linkname NewPath example.com/facade/internal/paths.NewPath
func NewPath(directory string, options raw.Options) string
//go:linkname IsSensitive example.com/facade/internal/paths.IsSensitive
func IsSensitive(options raw.Options) bool
`))
	providerHelpers, adapters, err := splitShimHelpers(map[string][]byte{
		"paths/compatibility.go": []byte(`package paths

import raw "example.com/facade/internal/paths"

type Sensitivity uint8
const CaseSensitive Sensitivity = 1
func NewPath(directory string, sensitivity Sensitivity) string {
    return raw.NewPath(directory, raw.Options{Sensitive: sensitivity == CaseSensitive})
}
`),
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := writeShimHelpers(providerRoot, providerHelpers); err != nil {
		t.Fatal(err)
	}
	if err := generateBackport(providerRoot, backportRoot, "example.com/facade/internal/", "example.com/facade/provider", adapters); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(providerRoot, "paths", "compatibility.go")); !os.IsNotExist(err) {
		t.Fatalf("compatibility adapters must not be installed into provider shims: %v", err)
	}
	writeTestFile(t, filepath.Join(backportRoot, "paths", "compatibility_test.go"), []byte(`package paths

import (
    "testing"
    provider "example.com/facade/provider/paths"
)

func TestProviderAndCanonicalAPIs(t *testing.T) {
    var options Options = provider.Options{Sensitive: true}
    if provider.NewPath("/project", options) != "/project:sensitive" {
        t.Fatal("provider must preserve its original API")
    }
    if NewPath("/project", CaseSensitive) != "/project:sensitive" || !IsSensitive(options) {
        t.Fatal("canonical facade must adapt the API and preserve provider type identity")
    }
}
`))
	command := exec.Command("go", "test", "./...")
	command.Dir = root
	command.Env = append(os.Environ(), "GOWORK=off")
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("compile and exercise provider and canonical APIs: %v\n%s", err, output)
	}
}
