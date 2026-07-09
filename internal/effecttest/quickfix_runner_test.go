package effecttest_test

import (
	"path/filepath"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/effecttest"

	// Register fourslash VFS callback to mount Effect packages
	_ "github.com/effect-ts/tsgo/etstesthooks"
	// Register Effect code fix provider for LSP code actions
	_ "github.com/effect-ts/tsgo/etslshooks"
)

func TestEffectQuickFixes(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV4, "effect"); err != nil {
		t.Skip("Effect not installed:", err)
	}

	cases, err := effecttest.DiscoverTestCases(bundledeffect.EffectV4)
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
			effecttest.RunEffectQuickFixTest(t, bundledeffect.EffectV4, tc)
		})
	}
}

func TestEffectV3QuickFixes(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV3, "effect"); err != nil {
		t.Skip("Effect V3 not installed:", err)
	}

	cases, err := effecttest.DiscoverTestCases(bundledeffect.EffectV3)
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
			effecttest.RunEffectQuickFixTest(t, bundledeffect.EffectV3, tc)
		})
	}
}
