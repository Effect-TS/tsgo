package effecttest_test

import (
	"path/filepath"
	"strings"
	"testing"

	"github.com/effect-ts/effect-typescript-go/internal/effecttest"

	_ "github.com/effect-ts/effect-typescript-go/etslshooks"
)

func TestEffectDocumentSymbols(t *testing.T) {
	t.Parallel()
	if err := effecttest.EnsureEffectInstalled(effecttest.EffectV4); err != nil {
		t.Skip("Effect not installed:", err)
	}

	cases, err := effecttest.DiscoverDocumentSymbolTestCases(effecttest.EffectV4)
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
			effecttest.RunEffectDocumentSymbolsTest(t, effecttest.EffectV4, tc)
		})
	}
}

func TestEffectV3DocumentSymbols(t *testing.T) {
	t.Parallel()
	if err := effecttest.EnsureEffectInstalled(effecttest.EffectV3); err != nil {
		t.Skip("Effect V3 not installed:", err)
	}

	cases, err := effecttest.DiscoverDocumentSymbolTestCases(effecttest.EffectV3)
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
			effecttest.RunEffectDocumentSymbolsTest(t, effecttest.EffectV3, tc)
		})
	}
}
