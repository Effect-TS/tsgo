package rules_test

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"html"
	"maps"
	"os"
	"path/filepath"
	"reflect"
	"regexp"
	"runtime"
	"slices"
	"sort"
	"strings"
	"testing"
	"testing/fstest"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/effecttest"
	"github.com/effect-ts/tsgo/internal/fixables"
	"github.com/effect-ts/tsgo/internal/pluginoptions"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/bundled"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/parser"
	"github.com/microsoft/typescript-go/shim/tsoptions"
	"github.com/microsoft/typescript-go/shim/tspath"
	"github.com/microsoft/typescript-go/shim/vfs"
	"github.com/microsoft/typescript-go/shim/vfs/vfstest"

	// Import etscheckerhooks to register Effect diagnostic callbacks
	_ "github.com/effect-ts/tsgo/etscheckerhooks"
)

func TestUpdateReadme(t *testing.T) {
	t.Parallel()
	if os.Getenv("UPDATE_README") == "" {
		t.Skip("set UPDATE_README=1 to regenerate README.md")
	}
	root := repoRoot(t)
	readmePath := filepath.Join(root, "README.md")
	committed, err := os.ReadFile(readmePath)
	if err != nil {
		t.Fatalf("read README.md: %v", err)
	}
	generated, err := generateReadme(committed)
	if err != nil {
		t.Fatalf("generate README: %v", err)
	}
	if err := os.WriteFile(readmePath, generated, 0o644); err != nil {
		t.Fatalf("write README.md: %v", err)
	}
	t.Logf("README.md updated")
}

func TestReadmeTable(t *testing.T) {
	t.Parallel()
	root := repoRoot(t)
	localPath := filepath.Join(root, "testdata", "baselines", "local", "README.md")
	referencePath := filepath.Join(root, "README.md")

	committed, err := os.ReadFile(referencePath)
	if err != nil {
		t.Fatalf("read README.md: %v", err)
	}

	got, err := generateReadme(committed)
	if err != nil {
		t.Fatalf("generate README: %v", err)
	}

	if !bytes.Equal(got, committed) {
		if err := os.MkdirAll(filepath.Dir(localPath), 0o755); err != nil {
			t.Fatalf("create local baseline dir: %v", err)
		}
		if err := os.WriteFile(localPath, got, 0o644); err != nil {
			t.Fatalf("write local baseline: %v", err)
		}
		t.Fatalf("README.md diagnostics table mismatch:\nlocal: %s\nreference: %s", localPath, referencePath)
	}
}

func TestMetadataJSON(t *testing.T) {
	t.Parallel()
	root := repoRoot(t)
	localPath := filepath.Join(root, "testdata", "baselines", "local", "metadata.json")
	referencePath := filepath.Join(root, "_packages", "tsgo", "src", "metadata.json")

	got, err := marshalMetadataJSON(t)
	if err != nil {
		t.Fatalf("marshal metadata.json: %v", err)
	}

	want, err := os.ReadFile(referencePath)
	if err != nil {
		t.Fatalf("read reference metadata.json at %s: %v", referencePath, err)
	}
	if !bytes.Equal(got, want) {
		if err := os.MkdirAll(filepath.Dir(localPath), 0o755); err != nil {
			t.Fatalf("create local baseline dir: %v", err)
		}
		if err := os.WriteFile(localPath, got, 0o644); err != nil {
			t.Fatalf("write local baseline: %v", err)
		}
		t.Fatalf("metadata.json mismatch:\nlocal: %s\nreference: %s", localPath, referencePath)
	}
}

func TestUpdateMetadataJSON(t *testing.T) {
	t.Parallel()
	if os.Getenv("UPDATE_METADATA_JSON") == "" {
		t.Skip("set UPDATE_METADATA_JSON=1 to regenerate metadata.json")
	}
	root := repoRoot(t)
	metadataPath := filepath.Join(root, "_packages", "tsgo", "src", "metadata.json")
	generated, err := marshalMetadataJSON(t)
	if err != nil {
		t.Fatalf("marshal metadata.json: %v", err)
	}
	if err := os.WriteFile(metadataPath, generated, 0o644); err != nil {
		t.Fatalf("write metadata.json: %v", err)
	}
	t.Logf("metadata.json updated")
}

func TestRuleDocs(t *testing.T) {
	t.Parallel()
	root := repoRoot(t)
	docsPath := filepath.Join(root, "docs", "rules")
	metadata := readMetadataDocument(t, root)
	generated := generateRuleDocs(metadata)

	entries, err := os.ReadDir(docsPath)
	if err != nil {
		t.Fatalf("read generated rule docs: %v", err)
	}
	for name, want := range generated {
		got, err := os.ReadFile(filepath.Join(docsPath, name))
		if err != nil {
			t.Fatalf("read generated rule doc %s: %v", name, err)
		}
		if string(got) != want {
			t.Fatalf("generated rule doc %s is out of date; run UPDATE_RULE_DOCS=1 go test ./internal/rules -run TestUpdateRuleDocs", name)
		}
	}
	for _, entry := range entries {
		if entry.IsDir() || filepath.Ext(entry.Name()) != ".md" {
			continue
		}
		if _, ok := generated[entry.Name()]; !ok {
			t.Fatalf("stale generated rule doc %s", entry.Name())
		}
	}
	for _, guide := range []struct {
		path    string
		heading string
	}{
		{path: filepath.Join(root, "README.md"), heading: "## Installation"},
		{path: filepath.Join(root, "docs", "README.md"), heading: "# Oxlint Setup"},
	} {
		content, err := os.ReadFile(guide.path)
		if err != nil {
			t.Fatalf("read rule docs guide %s: %v", guide.path, err)
		}
		if !strings.Contains(string(content), guide.heading) {
			t.Fatalf("rule docs guide %s is missing heading %q", guide.path, guide.heading)
		}
	}
}

func TestAnnotatePreview(t *testing.T) {
	t.Parallel()
	got := annotatePreview("const value = bad\n", []previewDiagnostic{{
		Start: 14,
		End:   17,
		Text:  "This value is not allowed. effect(exampleRule)",
	}}, "exampleRule")
	want := "const value = bad\n/**\n              ^^^ effecttsgo(example-rule): This value is not allowed.\n*/"
	if got != want {
		t.Fatalf("annotated preview mismatch\ngot:\n%s\nwant:\n%s", got, want)
	}

	got = annotatePreview("é bad(\n  value\n)", []previewDiagnostic{{
		Start: 2,
		End:   14,
		Text:  "This spans lines. effect(exampleRule)",
	}}, "exampleRule")
	want = "é bad(\n/**\n  ^^^^ effecttsgo(example-rule): This spans lines.\n*/\n  value\n/**\n^^^^^^^\n*/\n)"
	if got != want {
		t.Fatalf("multiline annotated preview mismatch\ngot:\n%s\nwant:\n%s", got, want)
	}
}

func TestMarkdownCodeFence(t *testing.T) {
	t.Parallel()
	if got, want := markdownCodeFence("const value = \"```\""), "````"; got != want {
		t.Fatalf("markdown fence mismatch: got %q, want %q", got, want)
	}
}

func TestUpdateRuleDocs(t *testing.T) { //nolint:paralleltest // Updates shared generated files.
	if os.Getenv("UPDATE_RULE_DOCS") == "" {
		t.Skip("set UPDATE_RULE_DOCS=1 to regenerate docs/rules")
	}
	root := repoRoot(t)
	docsPath := filepath.Join(root, "docs", "rules")
	if err := os.MkdirAll(docsPath, 0o755); err != nil {
		t.Fatalf("create rule docs directory: %v", err)
	}
	generated := generateRuleDocs(generateMetadataDocument(t))
	entries, err := os.ReadDir(docsPath)
	if err != nil {
		t.Fatalf("read rule docs directory: %v", err)
	}
	for _, entry := range entries {
		if !entry.IsDir() && filepath.Ext(entry.Name()) == ".md" {
			if err := os.Remove(filepath.Join(docsPath, entry.Name())); err != nil {
				t.Fatalf("remove stale rule doc %s: %v", entry.Name(), err)
			}
		}
	}
	names := slices.Sorted(maps.Keys(generated))
	for _, name := range names {
		if err := os.WriteFile(filepath.Join(docsPath, name), []byte(generated[name]), 0o644); err != nil {
			t.Fatalf("write generated rule doc %s: %v", name, err)
		}
	}
	t.Logf("updated %d rule docs", len(generated))
}

func repoRoot(t *testing.T) string {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("resolve caller path")
	}
	return filepath.Clean(filepath.Join(filepath.Dir(file), "..", ".."))
}

type previewDiagnostic struct {
	Start int    `json:"start"`
	End   int    `json:"end"`
	Text  string `json:"text"`
}

type previewPayload struct {
	SourceText  string              `json:"sourceText"`
	Diagnostics []previewDiagnostic `json:"diagnostics"`
}

type exportedRule struct {
	Name            string           `json:"name"`
	Group           string           `json:"group"`
	Description     string           `json:"description"`
	DefaultSeverity etscore.Severity `json:"defaultSeverity"`
	Fixable         bool             `json:"fixable"`
	SupportedEffect []string         `json:"supportedEffect"`
	Codes           []int32          `json:"codes"`
	Preview         *previewPayload  `json:"preview,omitempty"`
}

type metadataDocument struct {
	Groups  []rule.MetadataGroup  `json:"groups"`
	Presets []rule.MetadataPreset `json:"presets"`
	Rules   []exportedRule        `json:"rules"`
}

// buildFixableCodes returns a set of diagnostic codes that have non-disable fixables.
func buildFixableCodes() map[int32]bool {
	result := make(map[int32]bool)
	for _, f := range fixables.All {
		if f.Name == "effectDisable" {
			continue
		}
		for _, code := range f.ErrorCodes {
			result[code] = true
		}
	}
	return result
}

// isRuleFixable returns true if any of the rule's codes has a non-disable fixable.
func isRuleFixable(codes []int32, fixableCodes map[int32]bool) bool {
	for _, code := range codes {
		if fixableCodes[code] {
			return true
		}
	}
	return false
}

// trimLeadingDirectives strips leading lines starting with "// @" from sourceText
// and returns the trimmed text along with the number of characters removed (including newlines).
// This matches the upstream trimLeadingDirectives logic used for preview generation.
func trimLeadingDirectives(sourceText string) (trimmed string, removedChars int) {
	lines := strings.Split(sourceText, "\n")
	index := 0

	for index < len(lines) {
		if !strings.HasPrefix(lines[index], "// @") {
			break
		}
		removedChars += len(lines[index])
		if index < len(lines)-1 {
			removedChars++ // newline character
		}
		index++
	}

	return strings.Join(lines[index:], "\n"), removedChars
}

// findPreviewFile locates the preview fixture for a rule, checking v4 first then v3.
// Returns the version, file path, and source text.
func findPreviewFile(root string, ruleName string) (bundledeffect.EffectVersion, string, string, error) {
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV4, bundledeffect.EffectV3} {
		filePath := filepath.Join(root, "testdata", "tests", string(version), ruleName+"_preview.ts")
		data, err := os.ReadFile(filePath)
		if err == nil {
			return version, filePath, string(data), nil
		}
	}
	return "", "", "", fmt.Errorf("no preview file found for rule %s", ruleName)
}

// parseTestConfig extracts a @test-config JSON object from source comments.
// Returns a map of extra config to merge into the Effect plugin options.
func parseTestConfig(sourceText string) map[string]any {
	re := regexp.MustCompile(`//\s*@test-config\s+(.+)`)
	match := re.FindStringSubmatch(sourceText)
	if match == nil {
		return nil
	}
	var config map[string]any
	if err := json.Unmarshal([]byte(match[1]), &config); err != nil {
		return nil
	}
	return config
}

// buildTsConfigWithTestConfig creates a tsconfig JSON string that merges
// the default Effect plugin config with any @test-config overrides.
func buildTsConfigWithTestConfig(testConfig map[string]any) string {
	plugin := map[string]any{
		"name":                            "@effect/language-service",
		"ignoreEffectErrorsInTscExitCode": true,
		"skipDisabledOptimization":        true,
	}
	if testConfig != nil {
		maps.Copy(plugin, testConfig)
	}
	tsConfig := map[string]any{
		"compilerOptions": map[string]any{
			"plugins": []any{plugin},
		},
	}
	data, err := json.Marshal(tsConfig)
	if err != nil {
		panic(err)
	}
	return string(data)
}

// evaluatePreview creates an in-memory program for a preview fixture and runs
// the specified rule directly to collect diagnostics. We bypass the checker hooks
// because preview files use "// @effect-diagnostics *:off" which causes the
// hook to early-return. Instead, we run the rule directly via rule.Run().
func evaluatePreview(t *testing.T, version bundledeffect.EffectVersion, sourceText string, r *rule.Rule) *previewPayload {
	t.Helper()

	effecttest.AcquireProgram()
	defer effecttest.ReleaseProgram()

	// Parse test units (handles @filename directives for multi-file tests)
	defaultFileName := "preview.ts"
	units := parsePreviewUnits(sourceText, defaultFileName)

	// Create VFS
	testfs := make(map[string]any)
	if err := bundledeffect.MountEffect(version, testfs); err != nil {
		t.Fatalf("mount effect for preview: %v", err)
	}
	if strings.Contains(sourceText, "vitest") {
		if err := bundledeffect.MountVitest(version, testfs); err != nil {
			t.Fatalf("mount vitest for preview: %v", err)
		}
	}

	currentDirectory := "/.src"

	// Add test files to VFS
	var programFileNames []string

	for _, unit := range units {
		unitName := tspath.GetNormalizedAbsolutePath(unit.name, currentDirectory)
		testfs[unitName] = &fstest.MapFile{
			Data: []byte(unit.content),
		}
		if strings.HasPrefix(unitName, "/node_modules/") {
			continue
		}
		programFileNames = append(programFileNames, unitName)
	}

	// Inject tsconfig with optional @test-config overrides
	testConfig := parseTestConfig(sourceText)
	var tsConfigContent string
	if testConfig != nil {
		tsConfigContent = buildTsConfigWithTestConfig(testConfig)
	} else {
		tsConfigContent = effecttest.DefaultTsConfig
	}
	tsConfigName := tspath.GetNormalizedAbsolutePath("tsconfig.json", currentDirectory)
	testfs[tsConfigName] = &fstest.MapFile{
		Data: []byte(tsConfigContent),
	}
	tsConfigPath := tspath.ToPath(tsConfigName, currentDirectory, true)
	configJSON := parser.ParseSourceFile(ast.SourceFileParseOptions{
		FileName: tsConfigName,
		Path:     tsConfigPath,
	}, tsConfigContent, core.ScriptKindJSON)
	tsConfigFile := &tsoptions.TsConfigSourceFile{
		SourceFile: configJSON,
	}

	// Create VFS
	fs := vfstest.FromMap(testfs, true)
	fs = bundled.WrapFS(fs)

	// Setup compiler options
	compilerOptions := &core.CompilerOptions{
		NewLine:                      core.NewLineKindLF,
		SkipDefaultLibCheck:          core.TSTrue,
		NoErrorTruncation:            core.TSTrue,
		Target:                       core.ScriptTargetESNext,
		Module:                       core.ModuleKindNodeNext,
		ModuleResolution:             core.ModuleResolutionKindNodeNext,
		ESModuleInterop:              core.TSTrue,
		AllowSyntheticDefaultImports: core.TSTrue,
	}

	// Parse tsconfig
	configDir := tspath.GetDirectoryPath("tsconfig.json")
	configDir = tspath.GetNormalizedAbsolutePath(configDir, currentDirectory)
	parseHost := &previewParseConfigHost{
		fs:               fs,
		currentDirectory: currentDirectory,
	}
	parsedConfig := tsoptions.ParseJsonSourceFileConfigFileContent(
		tsConfigFile,
		parseHost,
		configDir,
		nil, nil,
		tsConfigFile.SourceFile.FileName(),
		nil, nil, nil,
	)
	if parsedConfig.CompilerOptions() != nil {
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

	// Create compiler host
	host := compiler.NewCompilerHost(currentDirectory, fs, bundled.LibPath(), nil, nil)

	// Create program
	program := compiler.NewProgram(compiler.ProgramOptions{
		Config: &tsoptions.ParsedCommandLine{
			ParsedConfig: &core.ParsedOptions{
				CompilerOptions: compilerOptions,
				FileNames:       programFileNames,
			},
			ConfigFile: parsedConfig.ConfigFile,
		},
		Host:           host,
		SingleThreaded: core.TSTrue,
	})

	// Force full type-checking by calling GetSemanticDiagnostics first.
	// This ensures the checker processes all files, populating relation errors
	// and other type data needed by rules. The checker hooks won't emit Effect
	// diagnostics due to the *:off directive, but the type info will be available.
	ctx := context.Background()
	_ = program.GetSemanticDiagnostics(ctx, nil)

	// Now get the type checker and run the rule directly against each source file
	c, done := program.GetTypeChecker(ctx)
	defer done()

	var ruleDiags []previewDiagnostic
	for _, fileName := range programFileNames {
		sf := program.GetSourceFile(fileName)
		if sf == nil || sf.IsDeclarationFile {
			continue
		}
		var options *etscore.ResolvedEffectPluginOptions
		if parsedEffectConfig := program.Options().Effect; parsedEffectConfig != nil {
			options = pluginoptions.ResolveEffectPluginOptionsForSourceFile(
				parsedEffectConfig,
				sf.FileName(),
				program.Options().ConfigFilePath,
				program.UseCaseSensitiveFileNames(),
			)
		}
		ruleCtx := rule.NewContext(context.Background(), program, c, typeparser.NewTypeParser(program, c), sf, options, r.DefaultSeverity)
		diags := r.Run(ruleCtx)
		positionMap := sf.GetPositionMap()
		for _, diagnostic := range diags {
			ruleDiags = append(ruleDiags, previewDiagnostic{
				Start: positionMap.UTF8ToUTF16(diagnostic.Loc().Pos()),
				End:   positionMap.UTF8ToUTF16(diagnostic.Loc().End()),
				Text:  diagnostic.String(),
			})
		}
	}

	// Sort by start position
	sort.Slice(ruleDiags, func(i, j int) bool {
		return ruleDiags[i].Start < ruleDiags[j].Start
	})

	// Trim leading directives from source text
	trimmedSource, removedChars := trimLeadingDirectives(sourceText)
	removedUnits := utf16Length(sourceText[:removedChars])

	// Build preview diagnostics with adjusted offsets
	prevDiags := make([]previewDiagnostic, 0, len(ruleDiags))
	for _, d := range ruleDiags {
		start := d.Start - removedUnits
		end := d.End - removedUnits
		if start < 0 {
			start = 0
		}
		if end < 0 {
			end = 0
		}
		prevDiags = append(prevDiags, previewDiagnostic{
			Start: start,
			End:   end,
			Text:  d.Text,
		})
	}

	return &previewPayload{
		SourceText:  trimmedSource,
		Diagnostics: prevDiags,
	}
}

// previewParseConfigHost implements tsoptions.ParseConfigHost for preview VFS.
type previewParseConfigHost struct {
	fs               vfs.FS
	currentDirectory string
}

func (h *previewParseConfigHost) FS() vfs.FS {
	return h.fs
}

func (h *previewParseConfigHost) GetCurrentDirectory() string {
	return h.currentDirectory
}

// parsePreviewUnits parses a preview file into test units.
// Reuses the same logic as effecttest.parseTestUnits.
func parsePreviewUnits(content string, defaultFileName string) []previewUnit {
	lines := strings.Split(content, "\n")

	var units []previewUnit
	var currentContent strings.Builder
	var currentFileName string

	optionRegex := regexp.MustCompile(`(?m)^\/{2}\s*@(\w+)\s*:\s*([^\r\n]*)`)

	for _, line := range lines {
		if testMetaData := optionRegex.FindStringSubmatch(line); testMetaData != nil {
			metaDataName := strings.ToLower(testMetaData[1])
			if metaDataName == "filename" {
				if currentFileName != "" && currentContent.Len() > 0 {
					units = append(units, previewUnit{
						name:    currentFileName,
						content: currentContent.String(),
					})
				}
				currentFileName = strings.TrimSpace(testMetaData[2])
				currentContent.Reset()
				continue
			}
		}
		if currentContent.Len() > 0 {
			currentContent.WriteRune('\n')
		}
		currentContent.WriteString(line)
	}

	if currentFileName != "" {
		units = append(units, previewUnit{
			name:    currentFileName,
			content: currentContent.String(),
		})
	} else if currentContent.Len() > 0 {
		units = append(units, previewUnit{
			name:    defaultFileName,
			content: currentContent.String(),
		})
	}

	return units
}

type previewUnit struct {
	name    string
	content string
}

func marshalMetadataJSON(t *testing.T) ([]byte, error) {
	root := repoRoot(t)

	groups := rules.MetadataGroups()
	presets := rules.MetadataPresets()

	fixableCodes := buildFixableCodes()

	exported := make([]exportedRule, 0, len(rules.All))
	for i := range rules.All {
		current := &rules.All[i]
		codes := slices.Clone(current.Codes)
		slices.Sort(codes)
		fixable := isRuleFixable(codes, fixableCodes)

		// Find and evaluate preview file
		version, _, sourceText, err := findPreviewFile(root, current.Name)
		if err != nil {
			t.Fatalf("%v", err)
		}

		preview := evaluatePreview(t, version, sourceText, current)

		exported = append(exported, exportedRule{
			Name:            current.Name,
			Group:           current.Group,
			Description:     current.Description,
			DefaultSeverity: current.DefaultSeverity,
			Fixable:         fixable,
			SupportedEffect: current.SupportedEffect,
			Codes:           codes,
			Preview:         preview,
		})
	}
	groupOrder := make(map[string]int, len(groups))
	for i, g := range groups {
		groupOrder[g.ID] = i
	}
	slices.SortFunc(exported, func(a, b exportedRule) int {
		if ga, gb := groupOrder[a.Group], groupOrder[b.Group]; ga != gb {
			return ga - gb
		}
		return strings.Compare(a.Name, b.Name)
	})

	doc := metadataDocument{
		Groups:  groups,
		Presets: presets,
		Rules:   exported,
	}

	data, err := json.MarshalIndent(doc, "", "  ")
	if err != nil {
		return nil, err
	}
	return append(data, '\n'), nil
}

func generateReadmeTable() string {
	groups := rules.MetadataGroups()

	type ruleEntry struct {
		name        string
		group       string
		description string
	}

	allRules := make([]ruleEntry, 0, len(rules.All))
	for _, r := range rules.All {
		allRules = append(allRules, ruleEntry{
			name:        r.Name,
			group:       r.Group,
			description: r.Description,
		})
	}
	groupOrder := make(map[string]int, len(groups))
	for i, g := range groups {
		groupOrder[g.ID] = i
	}
	slices.SortFunc(allRules, func(a, b ruleEntry) int {
		if ga, gb := groupOrder[a.group], groupOrder[b.group]; ga != gb {
			return ga - gb
		}
		return strings.Compare(a.name, b.name)
	})

	var lines []string
	lines = append(lines, "<table>")
	lines = append(lines, "  <thead>")
	lines = append(lines, "    <tr><th>Rule</th><th>Description</th></tr>")
	lines = append(lines, "  </thead>")
	lines = append(lines, "  <tbody>")
	for _, group := range groups {
		var groupRules []ruleEntry
		for _, r := range allRules {
			if r.group == group.ID {
				groupRules = append(groupRules, r)
			}
		}
		if len(groupRules) == 0 {
			continue
		}
		lines = append(lines, fmt.Sprintf("    <tr><td colspan=\"2\"><strong>%s</strong> <em>%s</em></td></tr>",
			html.EscapeString(group.Name), html.EscapeString(group.Description)))
		for _, r := range groupRules {
			lines = append(lines, fmt.Sprintf("    <tr><td><a href=\"docs/rules/%s.md\"><code>%s</code></a></td><td>%s</td></tr>",
				kebabCase(r.name), html.EscapeString(r.name), html.EscapeString(r.description)))
		}
	}
	lines = append(lines, "  </tbody>")
	lines = append(lines, "</table>")
	return strings.Join(lines, "\n")
}

func readMetadataDocument(t *testing.T, root string) metadataDocument {
	t.Helper()
	path := filepath.Join(root, "_packages", "tsgo", "src", "metadata.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read metadata.json: %v", err)
	}
	var metadata metadataDocument
	if err := json.Unmarshal(data, &metadata); err != nil {
		t.Fatalf("decode metadata.json: %v", err)
	}
	return metadata
}

func generateMetadataDocument(t *testing.T) metadataDocument {
	t.Helper()
	data, err := marshalMetadataJSON(t)
	if err != nil {
		t.Fatalf("generate metadata.json: %v", err)
	}
	var metadata metadataDocument
	if err := json.Unmarshal(data, &metadata); err != nil {
		t.Fatalf("decode generated metadata.json: %v", err)
	}
	return metadata
}

func generateRuleDocs(metadata metadataDocument) map[string]string {
	groupNames := make(map[string]string, len(metadata.Groups))
	for _, group := range metadata.Groups {
		groupNames[group.ID] = group.Name
	}
	docs := make(map[string]string, len(metadata.Rules))
	for _, current := range metadata.Rules {
		slug := kebabCase(current.Name)
		var codes []string
		for _, code := range current.Codes {
			codes = append(codes, fmt.Sprintf("`TS%d`", code))
		}
		fixable := "No"
		if current.Fixable {
			fixable = "Yes"
		}
		var page strings.Builder
		fmt.Fprintf(&page, "<!-- This file is generated by internal/rules/rules_json_test.go. Do not edit it manually. -->\n\n")
		fmt.Fprintf(&page, "# `%s`\n\n%s\n\n", current.Name, current.Description)
		page.WriteString("| Property | Value |\n| --- | --- |\n")
		fmt.Fprintf(&page, "| Category | %s |\n", markdownTableCell(groupNames[current.Group]))
		fmt.Fprintf(&page, "| Default severity | `%s` |\n", current.DefaultSeverity)
		fmt.Fprintf(&page, "| Fixable | %s |\n", fixable)
		fmt.Fprintf(&page, "| Effect versions | %s |\n", strings.Join(current.SupportedEffect, ", "))
		fmt.Fprintf(&page, "| Diagnostic codes | %s |\n", strings.Join(codes, ", "))
		fmt.Fprintf(&page, "| Language Service name | `%s` |\n", current.Name)
		fmt.Fprintf(&page, "| Oxlint name | `effecttsgo/%s` |\n", slug)

		page.WriteString("\n## Preview\n\n")
		if current.Preview == nil {
			page.WriteString("No preview is available.\n")
		} else {
			annotated := annotatePreview(
				current.Preview.SourceText,
				current.Preview.Diagnostics,
				current.Name,
			)
			fence := markdownCodeFence(annotated)
			fmt.Fprintf(&page, "%sts\n%s\n%s\n", fence, annotated, fence)
			if len(current.Preview.Diagnostics) == 0 {
				page.WriteString("\n")
				page.WriteString("This rule depends on project context and does not emit a diagnostic in the standalone preview.\n")
			}
		}

		fmt.Fprintf(&page, "\n## Language Service Configuration\n\nSee the [Language Service setup guide](../../README.md#installation) for installation instructions.\n\n```jsonc\n{\n  \"compilerOptions\": {\n    \"plugins\": [\n      {\n        \"name\": \"@effect/language-service\",\n        \"diagnosticSeverity\": {\n          \"%s\": \"warning\"\n        }\n      }\n    ]\n  }\n}\n```\n", current.Name)
		fmt.Fprintf(&page, "\n## Oxlint Configuration\n\nSee the [Oxlint setup guide](../README.md#oxlint-setup) for installation and patching instructions.\n\n```json\n{\n  \"plugins\": [\"effecttsgo\"],\n  \"rules\": {\n    \"effecttsgo/%s\": \"warn\"\n  }\n}\n```\n", slug)
		docs[slug+".md"] = page.String()
	}
	return docs
}

func kebabCase(value string) string {
	var result strings.Builder
	for index := range len(value) {
		current := value[index]
		if current >= 'A' && current <= 'Z' {
			previousIsLower := index > 0 && value[index-1] >= 'a' && value[index-1] <= 'z'
			nextIsLower := index+1 < len(value) && value[index+1] >= 'a' && value[index+1] <= 'z'
			if index > 0 && (previousIsLower || nextIsLower) {
				result.WriteByte('-')
			}
			result.WriteByte(current + ('a' - 'A'))
		} else {
			result.WriteByte(current)
		}
	}
	return result.String()
}

func markdownTableCell(value string) string {
	return strings.ReplaceAll(strings.ReplaceAll(value, "\n", " "), "|", "\\|")
}

func annotatePreview(sourceText string, diagnostics []previewDiagnostic, ruleName string) string {
	type annotation struct {
		padding string
		width   int
		text    string
	}
	lines := strings.Split(sourceText, "\n")
	lineStarts := make([]int, len(lines))
	offset := 0
	for index, line := range lines {
		lineStarts[index] = offset
		offset += len(line) + 1
	}
	annotations := make(map[int][]annotation)
	seen := make(map[string]bool)
	seenMessages := make(map[string]bool)
	sortedDiagnostics := slices.Clone(diagnostics)
	slices.SortFunc(sortedDiagnostics, func(left, right previewDiagnostic) int {
		if left.Start != right.Start {
			return left.Start - right.Start
		}
		return left.End - right.End
	})
	for _, diagnostic := range sortedDiagnostics {
		start := utf16OffsetToByteIndex(sourceText, max(0, diagnostic.Start))
		end := utf16OffsetToByteIndex(sourceText, max(diagnostic.Start+1, diagnostic.End))
		text := strings.TrimSuffix(diagnostic.Text, " effect("+ruleName+")")
		text = strings.Join(strings.Fields(text), " ")
		messagePending := !seenMessages[text]
		seenMessages[text] = true
		for lineIndex, line := range lines {
			line = strings.TrimSuffix(line, "\r")
			lineStart := lineStarts[lineIndex]
			lineEnd := lineStart + len(line)
			segmentStart := max(start, lineStart)
			segmentEnd := min(end, lineEnd)
			if segmentEnd <= segmentStart {
				continue
			}
			byteColumn := segmentStart - lineStart
			padding := annotationPadding(line[:byteColumn])
			width := max(1, len([]rune(line[byteColumn:segmentEnd-lineStart])))
			annotationText := ""
			if messagePending {
				annotationText = text
				messagePending = false
			}
			key := fmt.Sprintf("%d:%s:%d:%s", lineIndex, padding, width, annotationText)
			if seen[key] {
				continue
			}
			seen[key] = true
			annotations[lineIndex] = append(annotations[lineIndex], annotation{
				padding: padding,
				width:   width,
				text:    annotationText,
			})
		}
	}
	var result strings.Builder
	for index, line := range lines {
		if index > 0 {
			result.WriteByte('\n')
		}
		result.WriteString(line)
		for _, current := range annotations[index] {
			result.WriteString("\n/**\n")
			result.WriteString(current.padding)
			result.WriteString(strings.Repeat("^", current.width))
			if current.text != "" {
				fmt.Fprintf(&result, " effecttsgo(%s): %s", kebabCase(ruleName), current.text)
			}
			result.WriteString("\n*/")
		}
	}
	return strings.Trim(result.String(), "\r\n")
}

func annotationPadding(value string) string {
	var padding strings.Builder
	for _, current := range value {
		if current == '\t' {
			padding.WriteByte('\t')
		} else {
			padding.WriteByte(' ')
		}
	}
	return padding.String()
}

func utf16OffsetToByteIndex(value string, offset int) int {
	units := 0
	for index, current := range value {
		if units >= offset {
			return index
		}
		units++
		if current > 0xffff {
			units++
		}
	}
	return len(value)
}

func utf16Length(value string) int {
	units := 0
	for _, current := range value {
		units++
		if current > 0xffff {
			units++
		}
	}
	return units
}

func markdownCodeFence(value string) string {
	longest := 0
	current := 0
	for _, character := range value {
		if character == '`' {
			current++
			longest = max(longest, current)
		} else {
			current = 0
		}
	}
	return strings.Repeat("`", max(3, longest+1))
}

func generateReadmeExampleConfig() string {
	typ := reflect.TypeFor[etscore.EffectPluginOptions]()
	type readmeEntry struct {
		name        string
		description string
		value       any
	}
	fields := reflect.VisibleFields(typ)
	entries := make([]readmeEntry, 0, len(fields))
	for _, field := range fields {
		name := jsonFieldName(field)
		if name == "" {
			continue
		}
		defaultValue := readmeDefaultValue(field)
		if defaultValue == nil {
			continue
		}
		entries = append(entries, readmeEntry{
			name:        name,
			description: field.Tag.Get("schema_description"),
			value:       defaultValue,
		})
	}

	var lines []string
	lines = append(lines, "```jsonc")
	lines = append(lines, "{")
	lines = append(lines, `  "compilerOptions": {`)
	lines = append(lines, `    "plugins": [`)
	lines = append(lines, `      {`)
	lines = append(lines, `        "name": "@effect/language-service",`)
	for i, entry := range entries {
		defaultText := compactJSON(entry.value)
		if entry.description != "" {
			lines = append(lines, fmt.Sprintf("        // %s (default: %s)", entry.description, defaultText))
		}
		encoded := indentedJSON(entry.value, "        ")
		comma := ","
		if i == len(entries)-1 {
			comma = ""
		}
		lines = append(lines, fmt.Sprintf(`        %q: %s%s`, entry.name, encoded, comma))
	}
	lines = append(lines, `      }`)
	lines = append(lines, `    ]`)
	lines = append(lines, `  }`)
	lines = append(lines, `}`)
	lines = append(lines, "```")

	return strings.Join(lines, "\n")
}

func readmeDefaultValue(field reflect.StructField) any {
	if defaultValue := decodeStructTagJSON(field, "schema_default"); defaultValue != nil {
		return defaultValue
	}
	switch field.Name {
	case "DiagnosticSeverity":
		return map[string]any{}
	case "KeyPatterns":
		return etscore.DefaultKeyPatterns
	case "Overrides":
		return []any{
			map[string]any{
				"include": []string{"src/**/*.ts"},
				"options": map[string]any{
					"diagnosticSeverity": map[string]any{
						"floatingEffect": "error",
					},
				},
			},
		}
	default:
		return nil
	}
}

func decodeStructTagJSON(field reflect.StructField, key string) any {
	value := field.Tag.Get(key)
	if value == "" {
		return nil
	}
	var decoded any
	if err := json.Unmarshal([]byte(value), &decoded); err != nil {
		panic(err)
	}
	return decoded
}

func jsonFieldName(field reflect.StructField) string {
	tag := field.Tag.Get("json")
	if tag == "" {
		return ""
	}
	name := strings.Split(tag, ",")[0]
	if name == "-" {
		return ""
	}
	return name
}

func compactJSON(value any) string {
	data, err := json.Marshal(value)
	if err != nil {
		panic(err)
	}
	return string(data)
}

func indentedJSON(value any, prefix string) string {
	data, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		panic(err)
	}
	parts := strings.Split(string(data), "\n")
	if len(parts) == 1 {
		return parts[0]
	}
	for i := 1; i < len(parts); i++ {
		parts[i] = prefix + parts[i]
	}
	return strings.Join(parts, "\n")
}

const (
	readmeStartMarker        = "<!-- diagnostics-table:start -->"
	readmeEndMarker          = "<!-- diagnostics-table:end -->"
	readmeExampleStartMarker = "<!-- example-config:start -->"
	readmeExampleEndMarker   = "<!-- example-config:end -->"
)

func generateReadme(committedReadme []byte) ([]byte, error) {
	content := string(committedReadme)
	diagnosticsStartIdx := strings.Index(content, readmeStartMarker)
	diagnosticsEndIdx := strings.Index(content, readmeEndMarker)
	if diagnosticsStartIdx < 0 || diagnosticsEndIdx < 0 || diagnosticsEndIdx <= diagnosticsStartIdx {
		return nil, errors.New("README.md missing diagnostics table markers")
	}
	exampleStartIdx := strings.Index(content, readmeExampleStartMarker)
	exampleEndIdx := strings.Index(content, readmeExampleEndMarker)
	if exampleStartIdx < 0 || exampleEndIdx < 0 || exampleEndIdx <= exampleStartIdx {
		return nil, errors.New("README.md missing example config markers")
	}

	table := generateReadmeTable()
	example := generateReadmeExampleConfig()
	content = replaceReadmeSection(content, readmeStartMarker, readmeEndMarker, table)
	content = replaceReadmeSection(content, readmeExampleStartMarker, readmeExampleEndMarker, example)

	return []byte(content), nil
}

func replaceReadmeSection(content string, startMarker string, endMarker string, body string) string {
	before, afterStart, ok := strings.Cut(content, startMarker)
	if !ok {
		panic("missing start marker: " + startMarker)
	}
	_, afterEnd, ok := strings.Cut(afterStart, endMarker)
	if !ok {
		panic("missing end marker: " + endMarker)
	}
	var buf strings.Builder
	buf.WriteString(before)
	buf.WriteString(startMarker)
	buf.WriteString("\n")
	buf.WriteString(body)
	buf.WriteString("\n")
	buf.WriteString(endMarker)
	buf.WriteString(afterEnd)
	return buf.String()
}
