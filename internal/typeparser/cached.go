package typeparser

import (
	"github.com/microsoft/typescript-go/shim/core"
)

// Cached checks the store for an existing value. On miss, it calls compute,
// stores the result, and returns it. This correctly caches zero/nil values
// as valid negative results.
func Cached[K comparable, V any](store *core.LinkStore[K, V], key K, compute func() V) V {
	if store.Has(key) {
		return *store.TryGet(key)
	}
	v := compute()
	*store.Get(key) = v
	return v
}
