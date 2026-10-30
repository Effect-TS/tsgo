package effecttest_test

import (
	"path/filepath"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/effecttest"
)

func runFixtureCases(
	t *testing.T,
	version bundledeffect.EffectVersion,
	discover func(bundledeffect.EffectVersion) ([]string, error),
	run func(*testing.T, bundledeffect.EffectVersion, string),
) {
	t.Helper()
	if err := bundledeffect.EnsurePackageInstalled(version, "effect"); err != nil {
		t.Skip("Effect not installed:", err)
	}

	cases, err := discover(version)
	if err != nil {
		t.Fatal("Failed to discover test cases:", err)
	}

	if len(cases) == 0 {
		t.Skip("No test cases found")
	}

	for _, tc := range cases {
		t.Run(strings.TrimSuffix(filepath.Base(tc), ".ts"), func(t *testing.T) {
			t.Parallel()
			run(t, version, tc)
		})
	}
}

func TestEffectDiagnostics(t *testing.T) {
	t.Parallel()
	runFixtureCases(t, bundledeffect.EffectV4, effecttest.DiscoverTestCases, effecttest.RunEffectTest)
}

func TestEffectV3Diagnostics(t *testing.T) {
	t.Parallel()
	runFixtureCases(t, bundledeffect.EffectV3, effecttest.DiscoverTestCases, effecttest.RunEffectTest)
}
