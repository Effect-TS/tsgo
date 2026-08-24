package typeparser

import (
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/core"
)

func TestCachedCachesNegativeValues(t *testing.T) {
	t.Parallel()

	t.Run("nil", func(t *testing.T) {
		t.Parallel()

		var store core.LinkStore[string, *int]
		calls := 0
		compute := func() *int {
			calls++
			return nil
		}

		if Cached(&store, "key", compute) != nil {
			t.Fatal("expected nil result")
		}
		if Cached(&store, "key", compute) != nil {
			t.Fatal("expected cached nil result")
		}
		if calls != 1 {
			t.Fatalf("expected nil result to be computed once, got %d calls", calls)
		}
	})

	t.Run("false", func(t *testing.T) {
		t.Parallel()

		var store core.LinkStore[string, bool]
		calls := 0
		compute := func() bool {
			calls++
			return false
		}

		if Cached(&store, "key", compute) {
			t.Fatal("expected false result")
		}
		if Cached(&store, "key", compute) {
			t.Fatal("expected cached false result")
		}
		if calls != 1 {
			t.Fatalf("expected false result to be computed once, got %d calls", calls)
		}
	})
}
