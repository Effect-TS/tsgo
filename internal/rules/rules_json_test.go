package rules_test

import (
	"bytes"
	"context"
	"encoding/json"
	"maps"
	"fmt"
	"html"
	"os"
	"path/filepath"
	"regexp"
	"runtime"
	"slices"
	"sort"
	"strings"
	"testing"
	"testing/fstest"

	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/effecttest"
	"github.com/effect-ts/effect-typescript-go/internal/fixables"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/rules"
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
	_ "github.com/effect-ts/effect-typescript-go/etscheckerhooks"
)

func TestUpdateReadme(t *testing.T) {
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

	if err := os.MkdirAll(filepath.Dir(localPath), 0o755); err != nil {
		t.Fatalf("create local baseline dir: %v", err)
	}
	if err := os.WriteFile(localPath, got, 0o644); err != nil {
		t.Fatalf("write local baseline: %v", err)
	}

	if !bytes.Equal(got, committed) {
		t.Fatalf("README.md diagnostics table mismatch:\nlocal: %s\nreference: %s", localPath, referencePath)
	}
}

func TestMetadataJSON(t *testing.T) {
	root := repoRoot(t)
	localPath := filepath.Join(root, "testdata", "baselines", "local", "metadata.json")
	referencePath := filepath.Join(root, "_packages", "tsgo", "src", "metadata.json")

	got, err := marshalMetadataJSON(t)
	if err != nil {
		t.Fatalf("marshal metadata.json: %v", err)
	}

	if err := os.MkdirAll(filepath.Dir(localPath), 0o755); err != nil {
		t.Fatalf("create local baseline dir: %v", err)
	}
	if err := os.WriteFile(localPath, got, 0o644); err != nil {
		t.Fatalf("write local baseline: %v", err)
	}

	want, err := os.ReadFile(referencePath)
	if err != nil {
		t.Fatalf("read reference metadata.json at %s: %v", referencePath, err)
	}
	if !bytes.Equal(got, want) {
		t.Fatalf("metadata.json mismatch:\nlocal: %s\nreference: %s", localPath, referencePath)
	}
}

func repoRoot(t *testing.T) string {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("resolve caller path")
	}
	return filepath.Clean(filepath.Join(filepath.Dir(file), "..", ".."))
}

type metadataGroup struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Description string `json:"description"`
}

type previewDiagnostic struct {
	Start int `json:"start"`
	End   int `json:"end"`
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
	Groups []metadataGroup `json:"groups"`
	Rules  []exportedRule  `json:"rules"`
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
			removedChars += 1 // newline character
		}
		index++
	}

	return strings.Join(lines[index:], "\n"), removedChars
}

// findPreviewFile locates the preview fixture for a rule, checking v4 first then v3.
// Returns the version, file path, and source text.
func findPreviewFile(root string, ruleName string) (effecttest.EffectVersion, string, string, error) {
	for _, version := range []effecttest.EffectVersion{effecttest.EffectV4, effecttest.EffectV3} {
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
	data, _ := json.Marshal(tsConfig)
	return string(data)
}

// evaluatePreview creates an in-memory program for a preview fixture and runs
// the specified rule directly to collect diagnostics. We bypass the checker hooks
// because preview files use "// @effect-diagnostics *:off" which causes the
// hook to early-return. Instead, we run the rule directly via rule.Run().
func evaluatePreview(t *testing.T, version effecttest.EffectVersion, sourceText string, r *rule.Rule) *previewPayload {
	t.Helper()

	effecttest.AcquireProgram()
	defer effecttest.ReleaseProgram()

	// Parse test units (handles @filename directives for multi-file tests)
	defaultFileName := "preview.ts"
	units := parsePreviewUnits(sourceText, defaultFileName)

	// Create VFS
	testfs := make(map[string]any)
	if err := effecttest.MountEffect(version, testfs); err != nil {
		t.Fatalf("mount effect for preview: %v", err)
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

	var ruleDiags []*ast.Diagnostic
	for _, fileName := range programFileNames {
		sf := program.GetSourceFile(fileName)
		if sf == nil || sf.IsDeclarationFile {
			continue
		}
		ruleCtx := rule.NewContext(c, sf, r.DefaultSeverity)
		diags := r.Run(ruleCtx)
		ruleDiags = append(ruleDiags, diags...)
	}

	// Sort by start position
	sort.Slice(ruleDiags, func(i, j int) bool {
		return ruleDiags[i].Loc().Pos() < ruleDiags[j].Loc().Pos()
	})

	// Trim leading directives from source text
	trimmedSource, removedChars := trimLeadingDirectives(sourceText)

	// Build preview diagnostics with adjusted offsets
	prevDiags := make([]previewDiagnostic, 0, len(ruleDiags))
	for _, d := range ruleDiags {
		start := int(d.Loc().Pos()) - removedChars
		end := int(d.Loc().End()) - removedChars
		if start < 0 {
			start = 0
		}
		if end < 0 {
			end = 0
		}
		prevDiags = append(prevDiags, previewDiagnostic{
			Start: start,
			End:   end,
			Text:  d.String(),
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

	groups := []metadataGroup{
		{ID: "correctness", Name: "Correctness", Description: "Wrong, unsafe, or structurally invalid code patterns."},
		{ID: "antipattern", Name: "Anti-pattern", Description: "Discouraged patterns that often lead to bugs or confusing behavior."},
		{ID: "effectNative", Name: "Effect-native", Description: "Prefer Effect-native APIs and abstractions when available."},
		{ID: "style", Name: "Style", Description: "Cleanup, consistency, and idiomatic Effect code."},
	}

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
			t.Logf("warning: %v", err)
		}

		var preview *previewPayload
		if err == nil {
			preview = evaluatePreview(t, version, sourceText, current)
		}

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
		Groups: groups,
		Rules:  exported,
	}

	data, err := json.MarshalIndent(doc, "", "  ")
	if err != nil {
		return nil, err
	}
	return append(data, '\n'), nil
}

func severityIcon(s etscore.Severity) string {
	switch s {
	case etscore.SeverityOff:
		return "➖"
	case etscore.SeverityError:
		return "❌"
	case etscore.SeverityWarning:
		return "⚠️"
	case etscore.SeverityMessage:
		return "💬"
	case etscore.SeveritySuggestion:
		return "💡"
	default:
		return "➖"
	}
}

func containsEffect(supported []string, version string) bool {
	for _, s := range supported {
		if s == version {
			return true
		}
	}
	return false
}

func generateReadmeTable() string {
	groups := []metadataGroup{
		{ID: "correctness", Name: "Correctness", Description: "Wrong, unsafe, or structurally invalid code patterns."},
		{ID: "antipattern", Name: "Anti-pattern", Description: "Discouraged patterns that often lead to bugs or confusing behavior."},
		{ID: "effectNative", Name: "Effect-native", Description: "Prefer Effect-native APIs and abstractions when available."},
		{ID: "style", Name: "Style", Description: "Cleanup, consistency, and idiomatic Effect code."},
	}

	fixableCodes := buildFixableCodes()

	type ruleEntry struct {
		name            string
		group           string
		description     string
		defaultSeverity etscore.Severity
		fixable         bool
		supportedEffect []string
	}

	allRules := make([]ruleEntry, 0, len(rules.All))
	for _, r := range rules.All {
		codes := slices.Clone(r.Codes)
		slices.Sort(codes)
		allRules = append(allRules, ruleEntry{
			name:            r.Name,
			group:           r.Group,
			description:     r.Description,
			defaultSeverity: r.DefaultSeverity,
			fixable:         isRuleFixable(codes, fixableCodes),
			supportedEffect: r.SupportedEffect,
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
	lines = append(lines, `    <tr><th>Diagnostic</th><th>Sev</th><th>Fix</th><th>Description</th><th>v3</th><th>v4</th></tr>`)
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
		lines = append(lines, fmt.Sprintf(`    <tr><td colspan="6"><strong>%s</strong> <em>%s</em></td></tr>`,
			html.EscapeString(group.Name), html.EscapeString(group.Description)))
		for _, r := range groupRules {
			fix := ""
			if r.fixable {
				fix = "🔧"
			}
			v3 := ""
			if containsEffect(r.supportedEffect, "v3") {
				v3 = "✓"
			}
			v4 := ""
			if containsEffect(r.supportedEffect, "v4") {
				v4 = "✓"
			}
			lines = append(lines, fmt.Sprintf(`    <tr><td><code>%s</code></td><td>%s</td><td>%s</td><td>%s</td><td>%s</td><td>%s</td></tr>`,
				html.EscapeString(r.name),
				severityIcon(r.defaultSeverity),
				fix,
				html.EscapeString(r.description),
				v3, v4))
		}
	}

	lines = append(lines, "  </tbody>")
	lines = append(lines, "</table>")
	lines = append(lines, "")
	lines = append(lines, "`➖` off by default, `❌` error, `⚠️` warning, `💬` message, `💡` suggestion, `🔧` quick fix available")

	return strings.Join(lines, "\n")
}

const readmeStartMarker = "<!-- diagnostics-table:start -->"
const readmeEndMarker = "<!-- diagnostics-table:end -->"

func generateReadme(committedReadme []byte) ([]byte, error) {
	content := string(committedReadme)
	startIdx := strings.Index(content, readmeStartMarker)
	endIdx := strings.Index(content, readmeEndMarker)
	if startIdx < 0 || endIdx < 0 || endIdx <= startIdx {
		return nil, fmt.Errorf("README.md missing diagnostics table markers")
	}

	table := generateReadmeTable()
	var buf strings.Builder
	buf.WriteString(content[:startIdx])
	buf.WriteString(readmeStartMarker)
	buf.WriteString("\n")
	buf.WriteString(table)
	buf.WriteString("\n")
	buf.WriteString(readmeEndMarker)
	buf.WriteString(content[endIdx+len(readmeEndMarker):])

	return []byte(buf.String()), nil
}
