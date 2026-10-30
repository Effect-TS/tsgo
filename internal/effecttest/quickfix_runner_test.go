package effecttest_test

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/effecttest"

	_ "github.com/effect-ts/tsgo/etslshooks"
	_ "github.com/effect-ts/tsgo/etstesthooks"
)

func TestEffectQuickFixes(t *testing.T) {
	t.Parallel()
	runFixtureCases(t, bundledeffect.EffectV4, effecttest.DiscoverTestCases, effecttest.RunEffectQuickFixTest)
}

func TestEffectV3QuickFixes(t *testing.T) {
	t.Parallel()
	runFixtureCases(t, bundledeffect.EffectV3, effecttest.DiscoverTestCases, effecttest.RunEffectQuickFixTest)
}
