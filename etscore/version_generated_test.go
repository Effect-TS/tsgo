package etscore

import (
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestEffectVersionMatchesPackageJSON(t *testing.T) {
	_, currentFile, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("failed to resolve test file path")
	}

	packageJSONPath := filepath.Join(filepath.Dir(currentFile), "..", "_packages", "tsgo", "package.json")
	content, err := os.ReadFile(packageJSONPath)
	if err != nil {
		t.Fatalf("failed to read %s: %v", packageJSONPath, err)
	}

	var pkg struct {
		Version string `json:"version"`
	}
	if err := json.Unmarshal(content, &pkg); err != nil {
		t.Fatalf("failed to parse %s: %v", packageJSONPath, err)
	}
	if pkg.Version == "" {
		t.Fatalf("package.json at %s has empty version", packageJSONPath)
	}

	if EffectVersion != pkg.Version {
		t.Fatalf("EffectVersion = %q, package.json version = %q", EffectVersion, pkg.Version)
	}
}
