package effecttest_test

import (
	"os"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/effecttest"
)

// TestScratchFixture runs the fixture at ETS_SCRATCH_FIXTURE, which lives
// outside testdata/tests, through the regular fixture runner, against Effect
// v3 when ETS_SCRATCH_EFFECT is "v3" and v4 otherwise. `repoctl pipings`
// drives it and removes the baselines it writes; without the variable it is
// skipped.
func TestScratchFixture(t *testing.T) {
	t.Parallel()
	fixture := os.Getenv("ETS_SCRATCH_FIXTURE")
	if fixture == "" {
		t.Skip("ETS_SCRATCH_FIXTURE is not set; run `pnpm exec repoctl pipings <file>`")
	}
	version := bundledeffect.EffectV4
	if os.Getenv("ETS_SCRATCH_EFFECT") == "v3" {
		version = bundledeffect.EffectV3
	}
	if err := bundledeffect.EnsurePackageInstalled(version, "effect"); err != nil {
		t.Fatal("Effect not installed:", err)
	}
	effecttest.RunEffectTest(t, version, fixture)
}
