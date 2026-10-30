package effecttest_test

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/effecttest"

	_ "github.com/effect-ts/tsgo/etslshooks"
	_ "github.com/effect-ts/tsgo/etstesthooks"
)

func TestEffectRefactors(t *testing.T) {
	t.Parallel()
	runFixtureCases(t, bundledeffect.EffectV4, effecttest.DiscoverRefactorTestCases, effecttest.RunEffectRefactorTest)
}

func TestEffectV3Refactors(t *testing.T) {
	t.Parallel()
	runFixtureCases(t, bundledeffect.EffectV3, effecttest.DiscoverRefactorTestCases, effecttest.RunEffectRefactorTest)
}
