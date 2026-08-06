package etscore_test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/testutil/baseline"
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
