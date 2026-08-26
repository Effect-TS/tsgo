package etscore_test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/testutil/baseline"
)

func TestGenerateOxlintSchemaMatchesReference(t *testing.T) {
	root := repoRoot(t)
	actual, err := generateOxlintSchema()
	if err != nil {
		t.Fatalf("generateOxlintSchema() error = %v", err)
	}

	localPath := filepath.Join(root, "testdata", "baselines", "local", "oxlint-schema.json")
	referencePath := filepath.Join(root, "oxlint-schema.json")

	writeIfChanged(t, localPath, actual)
	if os.Getenv("UPDATE_OXLINT_SCHEMA") == "1" {
		writeIfChanged(t, referencePath, actual)
		return
	}

	expected, err := os.ReadFile(referencePath)
	if err != nil {
		t.Fatalf("failed to read reference schema %q: %v", referencePath, err)
	}
	if bytes.Equal(actual, expected) {
		return
	}

	diff := baseline.DiffText(referencePath, localPath, string(expected), string(actual))
	diffLines := strings.Split(diff, "\n")
	for i := range diffLines {
		diffLines[i] = "  " + diffLines[i]
	}
	t.Fatalf("oxlint-schema.json is out of date:\n%s", strings.Join(diffLines, "\n"))
}

func TestGenerateOxlintPresetsMatchReferences(t *testing.T) {
	root := repoRoot(t)
	presets, err := generateOxlintPresets()
	if err != nil {
		t.Fatalf("generateOxlintPresets() error = %v", err)
	}

	referenceDirectory := filepath.Join(root, "oxlint-presets")
	localDirectory := filepath.Join(root, "testdata", "baselines", "local", "oxlint-presets")
	update := os.Getenv("UPDATE_OXLINT_PRESETS") == "1"

	for name, actual := range presets {
		referencePath := filepath.Join(referenceDirectory, name)
		if update {
			writeIfChanged(t, referencePath, actual)
			continue
		}

		expected, err := os.ReadFile(referencePath)
		if err != nil {
			t.Fatalf("failed to read reference preset %q: %v", referencePath, err)
		}
		if bytes.Equal(actual, expected) {
			continue
		}

		localPath := filepath.Join(localDirectory, name)
		writeIfChanged(t, localPath, actual)
		diff := baseline.DiffText(referencePath, localPath, string(expected), string(actual))
		t.Fatalf("%s is out of date:\n%s", referencePath, diff)
	}

	entries, err := os.ReadDir(referenceDirectory)
	if err != nil && !os.IsNotExist(err) {
		t.Fatalf("failed to read preset directory %q: %v", referenceDirectory, err)
	}
	for _, entry := range entries {
		if entry.IsDir() || filepath.Ext(entry.Name()) != ".json" {
			continue
		}
		if _, ok := presets[entry.Name()]; ok {
			continue
		}
		path := filepath.Join(referenceDirectory, entry.Name())
		if update {
			if err := os.Remove(path); err != nil {
				t.Fatalf("failed to remove stale preset %q: %v", path, err)
			}
			continue
		}
		t.Fatalf("stale Oxlint preset %q", path)
	}
}

type oxlintPreset struct {
	Options oxlintPresetOptions `json:"options"`
	Plugins []string            `json:"plugins"`
	Rules   map[string]string   `json:"rules"`
}

type oxlintPresetOptions struct {
	TypeAware bool `json:"typeAware"`
}

func generateOxlintPresets() (map[string][]byte, error) {
	groups := rules.MetadataGroups()
	generated := make(map[string][]byte, len(groups)+1)

	recommendedSeverities := make(map[string]string)
	for _, current := range rules.All {
		if severity := oxlintSeverity(current.DefaultSeverity); severity != "off" {
			recommendedSeverities[oxlintRuleName(current.Name)] = severity
		}
	}
	for _, preset := range rules.MetadataPresets() {
		for name, severity := range preset.DiagnosticSeverity {
			oxlintName := oxlintRuleName(name)
			if current, next := recommendedSeverities[oxlintName], oxlintSeverity(severity); current != "error" && next != "off" {
				recommendedSeverities[oxlintName] = next
			}
		}
	}
	if err := addOxlintPreset(generated, "recommended.json", recommendedSeverities); err != nil {
		return nil, err
	}

	for _, group := range groups {
		severities := make(map[string]string)
		for _, current := range rules.All {
			if current.Group == group.ID {
				severities[oxlintRuleName(current.Name)] = "warn"
			}
		}
		if err := addOxlintPreset(generated, oxlintRuleName(group.ID)+".json", severities); err != nil {
			return nil, err
		}
	}

	return generated, nil
}

func addOxlintPreset(generated map[string][]byte, name string, severities map[string]string) error {
	if _, exists := generated[name]; exists {
		return fmt.Errorf("duplicate Oxlint preset %q", name)
	}
	ruleNames := make([]string, 0, len(severities))
	for name := range severities {
		ruleNames = append(ruleNames, name)
	}
	slices.Sort(ruleNames)
	configuredRules := make(map[string]string, len(ruleNames))
	for _, name := range ruleNames {
		configuredRules["effecttsgo/"+name] = severities[name]
	}
	content, err := json.MarshalIndent(oxlintPreset{
		Options: oxlintPresetOptions{TypeAware: true},
		Plugins: []string{"effecttsgo"},
		Rules:   configuredRules,
	}, "", "  ")
	if err != nil {
		return err
	}
	generated[name] = append(content, '\n')
	return nil
}

func oxlintSeverity(severity etscore.Severity) string {
	switch severity {
	case etscore.SeverityOff, etscore.SeveritySkipFile:
		return "off"
	case etscore.SeverityError:
		return "error"
	case etscore.SeverityWarning, etscore.SeveritySuggestion, etscore.SeverityMessage:
		return "warn"
	default:
		panic(fmt.Sprintf("unsupported Effect severity %q", severity.String()))
	}
}

func generateOxlintSchema() ([]byte, error) {
	root := repoRootForGeneration()
	baseSchemaContent, err := os.ReadFile(filepath.Join(root, "_tools", "oxlint-configuration-base-schema.json"))
	if err != nil {
		return nil, err
	}

	var document map[string]any
	if err := json.Unmarshal(baseSchemaContent, &document); err != nil {
		return nil, err
	}
	definitions, ok := document["definitions"].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("definitions not found in base schema")
	}
	pluginOptions, ok := definitions["LintPluginOptionsSchema"].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("LintPluginOptionsSchema not found in base schema")
	}
	pluginNames, ok := pluginOptions["enum"].([]any)
	if !ok {
		return nil, fmt.Errorf("LintPluginOptionsSchema.enum not found in base schema")
	}
	for _, value := range pluginNames {
		if value == "effecttsgo" || value == "effect-tsgo" {
			return nil, fmt.Errorf("base schema already contains Effect plugin %q", value)
		}
	}
	pluginOptions["enum"] = append(pluginNames, "effecttsgo")

	ruleMap, ok := definitions["DummyRuleMap"].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("DummyRuleMap not found in base schema")
	}
	ruleProperties, ok := ruleMap["properties"].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("DummyRuleMap.properties not found in base schema")
	}
	for _, rule := range rules.All {
		name := "effecttsgo/" + oxlintRuleName(rule.Name)
		if _, exists := ruleProperties[name]; exists {
			return nil, fmt.Errorf("base schema already contains Effect rule %q", name)
		}
		ruleProperties[name] = map[string]any{"$ref": "#/definitions/RuleNoConfig"}
	}

	output, err := json.MarshalIndent(document, "", "  ")
	if err != nil {
		return nil, err
	}
	return append(output, '\n'), nil
}

func oxlintRuleName(value string) string {
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
