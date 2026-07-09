package effecttest_test

import (
	"path/filepath"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/effecttest"

	_ "github.com/effect-ts/tsgo/etslshooks"
)

func TestEffectDocumentSymbols(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV4, "effect"); err != nil {
		t.Skip("Effect not installed:", err)
	}

	cases, err := effecttest.DiscoverDocumentSymbolTestCases(bundledeffect.EffectV4)
	if err != nil {
		t.Fatal("Failed to discover document symbol test cases:", err)
	}

	if len(cases) == 0 {
		t.Skip("No Effect document symbol test cases found")
	}

	for _, tc := range cases {
		name := filepath.Base(tc)
		name = strings.TrimSuffix(name, ".ts")

		t.Run(name, func(t *testing.T) {
			t.Parallel()
			effecttest.RunEffectDocumentSymbolsTest(t, bundledeffect.EffectV4, tc)
		})
	}
}

func TestEffectV3DocumentSymbols(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV3, "effect"); err != nil {
		t.Skip("Effect V3 not installed:", err)
	}

	cases, err := effecttest.DiscoverDocumentSymbolTestCases(bundledeffect.EffectV3)
	if err != nil {
		t.Fatal("Failed to discover V3 document symbol test cases:", err)
	}

	if len(cases) == 0 {
		t.Skip("No Effect V3 document symbol test cases found")
	}

	for _, tc := range cases {
		name := filepath.Base(tc)
		name = strings.TrimSuffix(name, ".ts")

		t.Run(name, func(t *testing.T) {
			t.Parallel()
			effecttest.RunEffectDocumentSymbolsTest(t, bundledeffect.EffectV3, tc)
		})
	}
}
