package effecttest

import (
	"path/filepath"
	"strings"
	"testing"
)

func TestEffectDiagnostics(t *testing.T) {
	// Skip if Effect not installed
	if err := EnsureEffectInstalled(EffectV4); err != nil {
		t.Skip("Effect not installed:", err)
	}

	cases, err := DiscoverTestCases(EffectV4)
	if err != nil {
		t.Fatal("Failed to discover test cases:", err)
	}

	if len(cases) == 0 {
		t.Skip("No Effect test cases found")
	}

	for _, tc := range cases {
		name := filepath.Base(tc)
		name = strings.TrimSuffix(name, ".ts")

		t.Run(name, func(t *testing.T) {
			t.Parallel()
			RunEffectTest(t, EffectV4, tc)
		})
	}
}

func TestEffectV3Diagnostics(t *testing.T) {
	if err := EnsureEffectInstalled(EffectV3); err != nil {
		t.Skip("Effect V3 not installed:", err)
	}

	cases, err := DiscoverTestCases(EffectV3)
	if err != nil {
		t.Fatal("Failed to discover V3 test cases:", err)
	}

	if len(cases) == 0 {
		t.Skip("No Effect V3 test cases found")
	}

	for _, tc := range cases {
		name := filepath.Base(tc)
		name = strings.TrimSuffix(name, ".ts")

		t.Run(name, func(t *testing.T) {
			t.Parallel()
			RunEffectTest(t, EffectV3, tc)
		})
	}
}
