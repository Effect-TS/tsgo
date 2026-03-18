package rules

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"slices"
	"testing"

	"github.com/effect-ts/effect-typescript-go/etscore"
)

func TestRulesJSON(t *testing.T) {
	root := repoRoot(t)
	localPath := filepath.Join(root, "testdata", "baselines", "local", "rules.json")
	referencePath := filepath.Join(root, "_packages", "tsgo", "src", "rules.json")

	got, err := marshalRulesJSON()
	if err != nil {
		t.Fatalf("marshal rules.json: %v", err)
	}

	if err := os.MkdirAll(filepath.Dir(localPath), 0o755); err != nil {
		t.Fatalf("create local baseline dir: %v", err)
	}
	if err := os.WriteFile(localPath, got, 0o644); err != nil {
		t.Fatalf("write local baseline: %v", err)
	}

	want, err := os.ReadFile(referencePath)
	if err != nil {
		t.Fatalf("read reference rules.json at %s: %v", referencePath, err)
	}
	if !bytes.Equal(got, want) {
		t.Fatalf("rules.json mismatch:\nlocal: %s\nreference: %s", localPath, referencePath)
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

type exportedRule struct {
	Name            string           `json:"name"`
	Description     string           `json:"description"`
	DefaultSeverity etscore.Severity `json:"defaultSeverity"`
	Codes           []int32          `json:"codes"`
}

func marshalRulesJSON() ([]byte, error) {
	exported := make([]exportedRule, 0, len(All))
	for _, current := range All {
		codes := slices.Clone(current.Codes)
		slices.Sort(codes)
		exported = append(exported, exportedRule{
			Name:            current.Name,
			Description:     current.Description,
			DefaultSeverity: current.DefaultSeverity,
			Codes:           codes,
		})
	}
	slices.SortFunc(exported, func(a, b exportedRule) int {
		if a.Name < b.Name {
			return -1
		}
		if a.Name > b.Name {
			return 1
		}
		return 0
	})
	data, err := json.MarshalIndent(exported, "", "  ")
	if err != nil {
		return nil, err
	}
	return append(data, '\n'), nil
}
