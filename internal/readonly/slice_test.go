package readonly_test

import (
	"slices"
	"testing"

	"github.com/effect-ts/tsgo/internal/readonly"
)

func TestSliceOwnershipAndIteration(t *testing.T) {
	t.Parallel()
	backing := []int{2, 3, 5}
	view := readonly.New(backing)
	backing[1] = 7
	if got := view.Slice(1, 3).Copy(); !slices.Equal(got, []int{7, 5}) {
		t.Fatalf("borrowed subslice = %v, want [7 5]", got)
	}
	owned := view.Copy()
	owned[0] = 99
	appended := view.AppendTo([]int{1})
	appended[2] = 99
	var values, indexes []int
	for index, value := range view.All() {
		indexes = append(indexes, index)
		values = append(values, value)
	}
	if !slices.Equal(values, []int{2, 7, 5}) || !slices.Equal(indexes, []int{0, 1, 2}) {
		t.Fatalf("iteration returned %v at %v", values, indexes)
	}
	visits := 0
	for range view.All() {
		visits++
		break
	}
	if visits != 1 {
		t.Fatalf("early stop visited %d elements", visits)
	}
}

func TestSliceNilAndBounds(t *testing.T) {
	t.Parallel()
	var absent readonly.Slice[int]
	empty := readonly.New([]int{})
	if absent.Len() != 0 || !absent.IsNil() || absent.Copy() != nil || !absent.Slice(0, 0).IsNil() {
		t.Fatal("zero view lost its nil state")
	}
	if empty.Len() != 0 || empty.IsNil() || empty.Copy() == nil || empty.Slice(0, 0).IsNil() {
		t.Fatal("empty view became nil")
	}
	for _, operation := range []func(){
		func() { absent.At(0) },
		func() { empty.Slice(0, 1) },
		func() { readonly.New([]int{1}).At(-1) },
		func() { readonly.New([]int{1}).Slice(1, 0) },
	} {
		func() {
			defer func() {
				if recover() == nil {
					t.Error("out-of-bounds access did not panic")
				}
			}()
			operation()
		}()
	}
}
