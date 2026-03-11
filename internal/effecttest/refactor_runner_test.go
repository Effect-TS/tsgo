package effecttest_test

import (
	"path/filepath"
	"strings"
	"testing"

	"github.com/effect-ts/effect-typescript-go/internal/effecttest"

	// Register fourslash VFS callback to mount Effect packages
	_ "github.com/effect-ts/effect-typescript-go/etstesthooks"
	// Register Effect code fix and refactor providers for LSP code actions
	_ "github.com/effect-ts/effect-typescript-go/etslshooks"
)

func TestEffectRefactors(t *testing.T) {
	if err := effecttest.EnsureEffectInstalled(effecttest.EffectV4); err != nil {
		t.Skip("Effect not installed:", err)
	}

	cases, err := effecttest.DiscoverRefactorTestCases(effecttest.EffectV4)
	if err != nil {
		t.Fatal("Failed to discover refactor test cases:", err)
	}

	if len(cases) == 0 {
		t.Skip("No Effect refactor test cases found")
	}

	for _, tc := range cases {
		tc := tc
		name := filepath.Base(tc)
		name = strings.TrimSuffix(name, ".ts")

		t.Run(name, func(t *testing.T) {
			t.Parallel()
			effecttest.RunEffectRefactorTest(t, effecttest.EffectV4, tc)
		})
	}
}

func TestEffectV3Refactors(t *testing.T) {
	if err := effecttest.EnsureEffectInstalled(effecttest.EffectV3); err != nil {
		t.Skip("Effect V3 not installed:", err)
	}

	cases, err := effecttest.DiscoverRefactorTestCases(effecttest.EffectV3)
	if err != nil {
		t.Fatal("Failed to discover V3 refactor test cases:", err)
	}

	if len(cases) == 0 {
		t.Skip("No Effect V3 refactor test cases found")
	}

	for _, tc := range cases {
		tc := tc
		name := filepath.Base(tc)
		name = strings.TrimSuffix(name, ".ts")

		t.Run(name, func(t *testing.T) {
			t.Parallel()
			effecttest.RunEffectRefactorTest(t, effecttest.EffectV3, tc)
		})
	}
}
