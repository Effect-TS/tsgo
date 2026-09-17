package rules

import (
	"slices"
	"testing"

	"github.com/effect-ts/tsgo/etscore"
)

func TestStrictPreset(t *testing.T) {
	t.Parallel()

	strict := metadataPresetByName(t, "strict")

	for _, current := range All {
		severity, included := strict[current.Name]
		if current.DefaultSeverity == etscore.SeverityOff {
			if included {
				t.Errorf("off-by-default diagnostic %q is included", current.Name)
			}
			continue
		}
		if !included {
			t.Errorf("enabled-by-default diagnostic %q is missing", current.Name)
		} else if severity != etscore.SeverityError {
			t.Errorf("diagnostic %q has severity %q, want error", current.Name, severity)
		}
	}
}

func TestRecommendedPreset(t *testing.T) {
	t.Parallel()

	recommended := metadataPresetByName(t, "recommended")
	for _, current := range All {
		severity, included := recommended[current.Name]
		switch {
		case current.DefaultSeverity.IsOff():
			if included {
				t.Errorf("off-by-default diagnostic %q is included", current.Name)
			}
		case !included:
			t.Errorf("enabled-by-default diagnostic %q is missing", current.Name)
		case severity != current.DefaultSeverity:
			t.Errorf("diagnostic %q has severity %q, want default %q", current.Name, severity, current.DefaultSeverity)
		}
	}
}

func TestMetadataPresetCatalog(t *testing.T) {
	t.Parallel()

	presets := MetadataPresets()
	names := make([]string, len(presets))
	for index, preset := range presets {
		names[index] = preset.Name
	}
	want := []string{"recommended", "strict", "correctness", "antipattern", "effect-native", "style"}
	if !slices.Equal(names, want) {
		t.Fatalf("preset names = %v, want %v", names, want)
	}

	groups := map[string]string{
		"correctness":   "correctness",
		"antipattern":   "antipattern",
		"effect-native": "effectNative",
		"style":         "style",
	}
	for presetName, group := range groups {
		preset := metadataPresetByName(t, presetName)
		for _, current := range All {
			severity, included := preset[current.Name]
			if current.Group != group {
				if included {
					t.Errorf("preset %q includes diagnostic %q from group %q", presetName, current.Name, current.Group)
				}
				continue
			}
			if !included {
				t.Errorf("preset %q is missing diagnostic %q", presetName, current.Name)
			} else if severity != etscore.SeverityWarning {
				t.Errorf("preset %q diagnostic %q has severity %q, want warning", presetName, current.Name, severity)
			}
		}
	}
}

func metadataPresetByName(t *testing.T, name string) map[string]etscore.Severity {
	t.Helper()
	for _, preset := range MetadataPresets() {
		if preset.Name == name {
			return preset.DiagnosticSeverity
		}
	}
	t.Fatalf("preset %q not found", name)
	return nil
}
