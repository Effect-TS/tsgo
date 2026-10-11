// Package readonly provides borrowed slice views without writable element slots.
package readonly

import (
	"iter"
	"slices"
)

// Slice preserves the backing array and nil state of a borrowed slice.
// Elements are returned by value; pointers inside elements remain borrowed.
type Slice[T any] struct {
	items []T
}

// New borrows items. Its owner must not mutate published elements.
func New[T any](items []T) Slice[T] { return Slice[T]{items: items} }

func (s Slice[T]) Len() int { return len(s.items) }

func (s Slice[T]) At(index int) T { return s.items[index] }

func (s Slice[T]) Slice(start, end int) Slice[T] { return New(s.items[start:end]) }

func (s Slice[T]) IsNil() bool { return s.items == nil }

func (s Slice[T]) All() iter.Seq2[int, T] { return slices.All(s.items) }

// Copy returns a caller-owned slice, preserving nil versus empty.
func (s Slice[T]) Copy() []T { return slices.Clone(s.items) }

// AppendTo copies elements into the caller's destination.
func (s Slice[T]) AppendTo(dst []T) []T { return append(dst, s.items...) }
