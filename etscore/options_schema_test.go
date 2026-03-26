package etscore_test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/testutil/baseline"
)

type diagnosticRuleSchema struct {
	Name            string
	Description     string
	DefaultSeverity etscore.Severity
}

func TestGenerateTSConfigSchemaMatchesReference(t *testing.T) {
	root := repoRoot(t)
	actual, err := generateTSConfigSchema()
	if err != nil {
		t.Fatalf("generateTSConfigSchema() error = %v", err)
	}

	localPath := filepath.Join(root, "testdata", "baselines", "local", "schema.json")
	referencePath := filepath.Join(root, "schema.json")

	writeIfChanged(t, localPath, actual)

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
	t.Fatalf("schema.json is out of date:\n%s", strings.Join(diffLines, "\n"))
}

func repoRoot(t *testing.T) string {
	t.Helper()

	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("failed to resolve caller path")
	}
	return filepath.Dir(filepath.Dir(file))
}

func schemaRules() []diagnosticRuleSchema {
	result := make([]diagnosticRuleSchema, 0, len(rules.All))
	for _, rule := range rules.All {
		result = append(result, diagnosticRuleSchema{
			Name:            rule.Name,
			Description:     rule.Description,
			DefaultSeverity: rule.DefaultSeverity,
		})
	}
	return result
}

func generateTSConfigSchema() ([]byte, error) {
	root := repoRootForGeneration()
	baseSchemaPath := filepath.Join(root, "_tools", "tsconfig-base-schema.json")
	baseSchemaContent, err := os.ReadFile(baseSchemaPath)
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
	compilerOptionsDefinition, ok := definitions["compilerOptionsDefinition"].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("compilerOptionsDefinition not found in base schema")
	}
	compilerOptionsProperties, err := nestedObject(
		compilerOptionsDefinition,
		"properties",
		"compilerOptions",
		"properties",
	)
	if err != nil {
		return nil, err
	}
	plugins, ok := compilerOptionsProperties["plugins"].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("compilerOptions.plugins not found in base schema")
	}
	pluginItems, ok := plugins["items"].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("compilerOptions.plugins.items not found in base schema")
	}

	definitions["effectLanguageServicePluginOptionsDefinition"] = effectLanguageServicePluginOptionsSchema()
	definitions["effectLanguageServicePluginDiagnosticSeverityDefinition"] = diagnosticSeveritySchema()
	definitions["effectLanguageServicePluginSeverityDefinition"] = severitySchema()
	definitions["effectLanguageServicePluginKeyPatternDefinition"] = reflectedSchemaForStruct(reflect.TypeOf(etscore.KeyPattern{}))

	plugins["items"] = map[string]any{
		"anyOf": []any{
			effectLanguageServicePluginSchema(),
			otherPluginSchema(pluginItems),
		},
	}

	output, err := json.MarshalIndent(document, "", "  ")
	if err != nil {
		return nil, err
	}
	return append(output, '\n'), nil
}

func effectLanguageServicePluginSchema() map[string]any {
	optionsSchema := effectLanguageServicePluginOptionsSchema()
	properties := cloneProperties(optionsSchema["properties"].(map[string]any))
	schema := map[string]any{
		"type":                 "object",
		"title":                "@effect/language-service plugin",
		"additionalProperties": false,
		"properties":           properties,
		"required":             []string{"name"},
	}
	properties["name"] = map[string]any{
		"type":  "string",
		"const": etscore.EffectPluginName,
	}
	return schema
}

func effectLanguageServicePluginOptionsSchema() map[string]any {
	schema := reflectedSchemaForStruct(reflect.TypeOf(etscore.EffectPluginOptions{}))
	properties := schema["properties"].(map[string]any)
	properties["diagnosticSeverity"] = map[string]any{
		"$ref":        "#/definitions/effectLanguageServicePluginDiagnosticSeverityDefinition",
		"description": structFieldTag(reflect.TypeOf(etscore.EffectPluginOptions{}), "DiagnosticSeverity", "schema_description"),
		"default":     map[string]any{},
	}
	properties["keyPatterns"] = map[string]any{
		"type":        "array",
		"description": structFieldTag(reflect.TypeOf(etscore.EffectPluginOptions{}), "KeyPatterns", "schema_description"),
		"default":     etscore.DefaultKeyPatterns,
		"items": map[string]any{
			"$ref": "#/definitions/effectLanguageServicePluginKeyPatternDefinition",
		},
	}
	return schema
}

func diagnosticSeveritySchema() map[string]any {
	properties := map[string]any{}
	for _, rule := range schemaRules() {
		properties[rule.Name] = map[string]any{
			"$ref":        "#/definitions/effectLanguageServicePluginSeverityDefinition",
			"description": rule.Description,
			"default":     rule.DefaultSeverity.String(),
		}
	}
	return map[string]any{
		"type":        "object",
		"description": "Allows overriding the default severity for each Effect diagnostic across the project.",
		"additionalProperties": map[string]any{
			"type": "string",
			"enum": []string{
				etscore.SeverityOff.String(),
				etscore.SeverityError.String(),
				etscore.SeverityWarning.String(),
				etscore.SeverityMessage.String(),
				etscore.SeveritySuggestion.String(),
			},
		},
		"properties": properties,
	}
}

func severitySchema() map[string]any {
	return map[string]any{
		"type": "string",
		"enum": []string{
			etscore.SeverityOff.String(),
			etscore.SeverityError.String(),
			etscore.SeverityWarning.String(),
			etscore.SeverityMessage.String(),
			etscore.SeveritySuggestion.String(),
		},
	}
}

func otherPluginSchema(existing map[string]any) map[string]any {
	schema := cloneJSON(existing)
	properties, ok := schema["properties"].(map[string]any)
	if !ok {
		properties = map[string]any{}
		schema["properties"] = properties
	}

	nameSchema := map[string]any{
		"type":        "string",
		"description": "Plugin name.",
	}
	if existingNameSchema, ok := properties["name"].(map[string]any); ok {
		nameSchema = cloneJSON(existingNameSchema)
	}
	nameSchema["not"] = map[string]any{
		"enum": []string{etscore.EffectPluginName},
	}
	properties["name"] = nameSchema

	return schema
}

func reflectedSchemaForStruct(typ reflect.Type) map[string]any {
	properties := map[string]any{}
	for i := 0; i < typ.NumField(); i++ {
		field := typ.Field(i)
		name := jsonFieldName(field)
		if name == "" {
			continue
		}
		properties[name] = reflectedSchemaForField(field)
	}

	return map[string]any{
		"type":                 "object",
		"additionalProperties": false,
		"properties":           properties,
	}
}

func cloneProperties(properties map[string]any) map[string]any {
	cloned := make(map[string]any, len(properties))
	for key, value := range properties {
		cloned[key] = value
	}
	return cloned
}

func cloneJSON[T any](value T) T {
	data, err := json.Marshal(value)
	if err != nil {
		panic(err)
	}
	var cloned T
	if err := json.Unmarshal(data, &cloned); err != nil {
		panic(err)
	}
	return cloned
}

func reflectedSchemaForField(field reflect.StructField) map[string]any {
	schema := map[string]any{}
	fieldType := field.Type

	switch fieldType.Kind() {
	case reflect.Bool:
		schema["type"] = "boolean"
	case reflect.Int:
		schema["type"] = "integer"
	case reflect.String:
		schema["type"] = "string"
	case reflect.Slice:
		schema["type"] = "array"
		itemType := field.Tag.Get("schema_items_type")
		itemSchema := map[string]any{}
		if itemType != "" {
			itemSchema["type"] = itemType
		}
		if itemEnum := decodeJSONTag(field, "schema_items_enum"); itemEnum != nil {
			itemSchema["enum"] = itemEnum
		}
		if len(itemSchema) > 0 {
			schema["items"] = itemSchema
		}
	case reflect.Map:
		schema["type"] = "object"
		if propType := field.Tag.Get("schema_additional_properties_type"); propType != "" {
			schema["additionalProperties"] = map[string]any{"type": propType}
		}
	}

	if description := field.Tag.Get("schema_description"); description != "" {
		schema["description"] = description
	}
	if defaultValue := decodeJSONTag(field, "schema_default"); defaultValue != nil {
		schema["default"] = defaultValue
	}
	if enum := decodeJSONTag(field, "schema_enum"); enum != nil {
		schema["enum"] = enum
	}
	if minimum := decodeJSONTag(field, "schema_minimum"); minimum != nil {
		schema["minimum"] = minimum
	}
	if uniqueItems := decodeJSONTag(field, "schema_unique_items"); uniqueItems != nil {
		schema["uniqueItems"] = uniqueItems
	}

	return schema
}

func decodeJSONTag(field reflect.StructField, key string) any {
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

func structFieldTag(typ reflect.Type, fieldName string, tagName string) string {
	field, ok := typ.FieldByName(fieldName)
	if !ok {
		panic("missing field: " + fieldName)
	}
	return field.Tag.Get(tagName)
}

func repoRootForGeneration() string {
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		panic("failed to resolve caller path")
	}
	return filepath.Dir(filepath.Dir(file))
}

func nestedObject(root map[string]any, keys ...string) (map[string]any, error) {
	current := root
	for _, key := range keys {
		next, ok := current[key].(map[string]any)
		if !ok {
			return nil, fmt.Errorf("schema path %q not found", strings.Join(keys, "."))
		}
		current = next
	}
	return current, nil
}

func writeIfChanged(t *testing.T, path string, content []byte) {
	t.Helper()

	existing, err := os.ReadFile(path)
	if err == nil && bytes.Equal(existing, content) {
		return
	}
	if err != nil && !os.IsNotExist(err) {
		t.Fatalf("failed to read %q: %v", path, err)
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatalf("failed to create %q: %v", filepath.Dir(path), err)
	}
	if err := os.WriteFile(path, content, 0o644); err != nil {
		t.Fatalf("failed to write %q: %v", path, err)
	}
}
