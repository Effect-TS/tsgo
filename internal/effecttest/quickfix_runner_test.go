package effecttest_test

import (
	"path/filepath"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/effecttest"

	// Register fourslash VFS callback to mount Effect packages
	_ "github.com/effect-ts/tsgo/etstesthooks"
	// Register Effect code fix provider for LSP code actions
	_ "github.com/effect-ts/tsgo/etslshooks"
)

func TestEffectQuickFixes(t *testing.T) {
	t.Parallel()
	if err := effecttest.EnsureEffectInstalled(effecttest.EffectV4); err != nil {
		t.Skip("Effect not installed:", err)
	}

	cases, err := effecttest.DiscoverTestCases(effecttest.EffectV4)
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
			effecttest.RunEffectQuickFixTest(t, effecttest.EffectV4, tc)
		})
	}
}

func TestEffectV3QuickFixes(t *testing.T) {
	t.Parallel()
	if err := effecttest.EnsureEffectInstalled(effecttest.EffectV3); err != nil {
		t.Skip("Effect V3 not installed:", err)
	}

	cases, err := effecttest.DiscoverTestCases(effecttest.EffectV3)
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
			effecttest.RunEffectQuickFixTest(t, effecttest.EffectV3, tc)
		})
	}
}
