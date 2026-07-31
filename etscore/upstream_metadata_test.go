package etscore

import (
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

type upstreamMetadata struct {
	SchemaVersion int                     `json:"schemaVersion"`
	Next          upstreamProfileMetadata `json:"next"`
	Stable        upstreamProfileMetadata `json:"stable"`
	Oxlint        upstreamProfileMetadata `json:"oxlint"`
}

type upstreamProfileMetadata struct {
	TSVersion string `json:"tsVersion"`
	TSGitHead string `json:"tsGitHead"`
}

func TestUpstreamMetadataMatchesTypeScriptGoSubmodule(t *testing.T) {
	t.Parallel()

	repoRoot := repoRootFromCaller(t)
	upstreamJSONPath := filepath.Join(repoRoot, "_packages", "tsgo", "upstream.json")
	data, err := os.ReadFile(upstreamJSONPath)
	if err != nil {
		t.Fatalf("failed to read %s: %v", upstreamJSONPath, err)
	}

	var metadata upstreamMetadata
	if err := json.Unmarshal(data, &metadata); err != nil {
		t.Fatalf("failed to parse %s: %v", upstreamJSONPath, err)
	}
	if metadata.SchemaVersion != 1 {
		t.Fatalf("upstream.json at %s has unsupported schemaVersion %d", upstreamJSONPath, metadata.SchemaVersion)
	}

	profiles := map[string]upstreamProfileMetadata{
		"next":   metadata.Next,
		"stable": metadata.Stable,
		"oxlint": metadata.Oxlint,
	}
	for name, profile := range profiles {
		if profile.TSVersion == "" {
			t.Fatalf("upstream.json at %s has empty %s.tsVersion", upstreamJSONPath, name)
		}
		if profile.TSGitHead == "" {
			t.Fatalf("upstream.json at %s has empty %s.tsGitHead", upstreamJSONPath, name)
		}
	}

	cmd := exec.Command("git", "rev-parse", "HEAD:typescript-go")
	cmd.Dir = repoRoot
	output, err := cmd.Output()
	if err != nil {
		t.Fatalf("failed to read typescript-go gitlink: %v", err)
	}
	submoduleHead := strings.TrimSpace(string(output))

	matched := false
	for _, profile := range profiles {
		if profile.TSGitHead == submoduleHead {
			matched = true
			break
		}
	}
	if !matched {
		t.Fatalf("TypeScript-Go gitlink %s does not match any upstream profile", submoduleHead)
	}
}

func repoRootFromCaller(t *testing.T) string {
	t.Helper()

	_, currentFile, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("failed to resolve test file path")
	}
	return filepath.Join(filepath.Dir(currentFile), "..")
}
