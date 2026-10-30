package typeparser

import (
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
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

func withGlobalNodeId(node *ast.Node) *ast.Node {
	ast.GetNodeId(node)
	return node
}

func withGlobalSymbolId(symbol *ast.Symbol) *ast.Symbol {
	ast.GetSymbolId(symbol)
	return symbol
}

func TestCachedNodeCachesPerNode(t *testing.T) {
	t.Parallel()

	factory := ast.NewNodeFactory(ast.NodeFactoryHooks{})
	first := factory.NewIdentifier("first")
	second := factory.NewIdentifier("second")
	numbered := withGlobalNodeId(factory.NewIdentifier("numbered"))

	var store checker.NodeLinkStore[*string]
	calls := 0
	compute := func(value *string) func() *string {
		return func() *string {
			calls++
			return value
		}
	}
	secondValue, numberedValue := "second", "numbered"

	if store.Has(first) {
		t.Fatal("expected an empty store")
	}
	if CachedNode(&store, first, compute(nil)) != nil {
		t.Fatal("expected nil result for first")
	}
	if got := CachedNode(&store, second, compute(&secondValue)); got == nil || *got != "second" {
		t.Fatalf("expected %q for second, got %v", "second", got)
	}
	if got := CachedNode(&store, numbered, compute(&numberedValue)); got == nil || *got != "numbered" {
		t.Fatalf("expected %q for numbered, got %v", "numbered", got)
	}
	if CachedNode(&store, first, compute(&secondValue)) != nil {
		t.Fatal("expected cached nil result for first")
	}
	if got := CachedNode(&store, numbered, compute(nil)); got == nil || *got != "numbered" {
		t.Fatalf("expected cached %q for numbered, got %v", "numbered", got)
	}
	if !store.Has(first) || !store.Has(second) || !store.Has(numbered) {
		t.Fatal("expected every computed node to be present")
	}
	if calls != 3 {
		t.Fatalf("expected one computation per node, got %d calls", calls)
	}
}

func TestSymbolLinkStoreKeepsOneSlotPerSymbol(t *testing.T) {
	t.Parallel()

	first, second := new(ast.Symbol), new(ast.Symbol)
	numbered := withGlobalSymbolId(new(ast.Symbol))

	var store checker.SymbolLinkStore[string]
	if store.TryGet(first) != nil {
		t.Fatal("expected an empty store")
	}
	*store.Get(first) = "first"
	*store.Get(second) = "second"
	*store.Get(numbered) = "numbered"

	for symbol, want := range map[*ast.Symbol]string{first: "first", second: "second", numbered: "numbered"} {
		if got := store.TryGet(symbol); got == nil || *got != want {
			t.Fatalf("expected %q from TryGet, got %v", want, got)
		}
		if got := *store.Get(symbol); got != want {
			t.Fatalf("expected %q from Get, got %q", want, got)
		}
	}
}
