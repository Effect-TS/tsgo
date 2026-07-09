package effecttest_test

import (
	"path/filepath"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/effecttest"

	// Register fourslash VFS callback to mount Effect packages
	_ "github.com/effect-ts/tsgo/etstesthooks"
	// Register Effect code fix and refactor providers for LSP code actions
	_ "github.com/effect-ts/tsgo/etslshooks"
)

func TestEffectRefactors(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV4, "effect"); err != nil {
		t.Skip("Effect not installed:", err)
	}

	cases, err := effecttest.DiscoverRefactorTestCases(bundledeffect.EffectV4)
	if err != nil {
		t.Fatal("Failed to discover refactor test cases:", err)
	}

	if len(cases) == 0 {
		t.Skip("No Effect refactor test cases found")
	}

	for _, tc := range cases {
		name := filepath.Base(tc)
		name = strings.TrimSuffix(name, ".ts")

		t.Run(name, func(t *testing.T) {
			t.Parallel()
			effecttest.RunEffectRefactorTest(t, bundledeffect.EffectV4, tc)
		})
	}
}

func TestEffectV3Refactors(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV3, "effect"); err != nil {
		t.Skip("Effect V3 not installed:", err)
	}

	cases, err := effecttest.DiscoverRefactorTestCases(bundledeffect.EffectV3)
	if err != nil {
		t.Fatal("Failed to discover V3 refactor test cases:", err)
	}

	if len(cases) == 0 {
		t.Skip("No Effect V3 refactor test cases found")
	}

	for _, tc := range cases {
		name := filepath.Base(tc)
		name = strings.TrimSuffix(name, ".ts")

		t.Run(name, func(t *testing.T) {
			t.Parallel()
			effecttest.RunEffectRefactorTest(t, bundledeffect.EffectV3, tc)
		})
	}
}
