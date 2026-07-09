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
	if metadata.TSVersion == "" {
		t.Fatalf("upstream.json at %s has empty tsVersion", upstreamJSONPath)
	}
	if metadata.TSGitHead == "" {
		t.Fatalf("upstream.json at %s has empty tsGitHead", upstreamJSONPath)
	}

	cmd := exec.Command("git", "rev-parse", "HEAD:typescript-go")
	cmd.Dir = repoRoot
	output, err := cmd.Output()
	if err != nil {
		t.Fatalf("failed to read typescript-go gitlink: %v", err)
	}
	submoduleHead := strings.TrimSpace(string(output))
	if metadata.TSGitHead != submoduleHead {
		t.Fatalf("upstream.json tsGitHead %s does not match typescript-go gitlink %s", metadata.TSGitHead, submoduleHead)
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
