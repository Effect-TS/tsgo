package effecttest_test

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/effecttest"

	_ "github.com/effect-ts/tsgo/etslshooks"
)

func TestEffectDocumentSymbols(t *testing.T) {
	t.Parallel()
	runFixtureCases(t, bundledeffect.EffectV4, effecttest.DiscoverDocumentSymbolTestCases, effecttest.RunEffectDocumentSymbolsTest)
}

func TestEffectV3DocumentSymbols(t *testing.T) {
	t.Parallel()
	runFixtureCases(t, bundledeffect.EffectV3, effecttest.DiscoverDocumentSymbolTestCases, effecttest.RunEffectDocumentSymbolsTest)
}
