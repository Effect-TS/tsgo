package bundledeffect

import (
	"encoding/json"
	"maps"
	"os"
	"path/filepath"
	"testing"
)

type packageJSON struct {
	Dependencies map[string]string `json:"dependencies"`
	Version      string            `json:"version"`
}

func TestEmbeddedFixturesMatchInstalledPackages(t *testing.T) {
	t.Parallel()
	for _, version := range []EffectVersion{EffectV3, EffectV4} {
		profile, ok := fixtures().manifest.Profiles[string(version)]
		if !ok {
			t.Fatalf("embedded fixtures do not contain %s", version)
		}

		root := EffectTsGoRootPath()
		requested := readPackageJSON(t, filepath.Join(root, "testdata", "tests", string(version), "package.json"))
		if !maps.Equal(profile.Requested, requested.Dependencies) {
			t.Fatalf("embedded requested dependencies for %s do not match package.json", version)
		}
		if !maps.EqualFunc(profile.Requested, profile.Resolved, func(_, _ string) bool { return true }) {
			t.Fatalf("embedded requested and resolved package sets for %s do not match", version)
		}
		for packageName, embeddedVersion := range profile.Resolved {
			installed := readPackageJSON(t, filepath.Join(PackagePath(version, packageName), "package.json"))
			if installed.Version != embeddedVersion {
				t.Fatalf("embedded %s version for %s is %s, installed %s", version, packageName, embeddedVersion, installed.Version)
			}
		}
	}
}

func readPackageJSON(t *testing.T, path string) packageJSON {
	t.Helper()
	content, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	var parsed packageJSON
	if err := json.Unmarshal(content, &parsed); err != nil {
		t.Fatalf("decode %s: %v", path, err)
	}
	return parsed
}
