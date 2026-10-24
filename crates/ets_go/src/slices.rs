//! The sorting and searching part of Go's `slices` package.
//!
//! `slices.SortFunc` / `slices.Sort` are the same pdqsort template as `sort.Slice` (see [`crate::sort`]), so
//! their tie order is Go's. The other `slices` helpers (`Contains`, `Index`, `Compact`, `Clone`, ...) have
//! direct Rust equivalents with identical results and are not wrapped here.

use std::cmp::Ordering;

use crate::sort::{SliceLess, bits_len, pdqsort};

/// Go's `slices.Sort` for a totally ordered element type (`cmp.Less(a, b)` is `a < b`).
pub fn sort<T: Ord>(x: &mut [T]) {
    let n = x.len();
    pdqsort(&mut SliceLess { data: x, less: |a: &T, b: &T| a < b }, 0, n, bits_len(n));
}

/// Go's `slices.Sort` on `[]float64`: `cmp.Less` orders NaNs before every other value.
pub fn sort_f64(x: &mut [f64]) {
    let n = x.len();
    let less = |a: &f64, b: &f64| *a < *b || (a.is_nan() && !b.is_nan());
    pdqsort(&mut SliceLess { data: x, less }, 0, n, bits_len(n));
}

/// Go's `slices.SortFunc(x, cmp)`. `cmp(a, b) < 0` in Go is `Ordering::Less` here.
pub fn sort_func<T>(x: &mut [T], mut cmp: impl FnMut(&T, &T) -> Ordering) {
    let n = x.len();
    let less = |a: &T, b: &T| cmp(a, b) == Ordering::Less;
    pdqsort(&mut SliceLess { data: x, less }, 0, n, bits_len(n));
}

/// Go's `slices.SortStableFunc`. A stable sort has one correct result, so this is Rust's stable sort.
pub fn sort_stable_func<T>(x: &mut [T], cmp: impl FnMut(&T, &T) -> Ordering) {
    x.sort_by(cmp);
}

/// Go's `slices.Sorted(seq)`: collect, then [`sort`].
pub fn sorted<T: Ord>(seq: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut s: Vec<T> = seq.into_iter().collect();
    sort(&mut s);
    s
}

/// Go's `slices.SortedFunc(seq, cmp)`: collect, then [`sort_func`].
pub fn sorted_func<T>(
    seq: impl IntoIterator<Item = T>,
    cmp: impl FnMut(&T, &T) -> Ordering,
) -> Vec<T> {
    let mut s: Vec<T> = seq.into_iter().collect();
    sort_func(&mut s, cmp);
    s
}

/// Go's `slices.SortedStableFunc(seq, cmp)`.
pub fn sorted_stable_func<T>(
    seq: impl IntoIterator<Item = T>,
    cmp: impl FnMut(&T, &T) -> Ordering,
) -> Vec<T> {
    let mut s: Vec<T> = seq.into_iter().collect();
    sort_stable_func(&mut s, cmp);
    s
}

/// Go's `slices.BinarySearch(x, target)`: the position where `target` is or would be inserted, and
/// whether it was found.
pub fn binary_search<T: Ord>(x: &[T], target: &T) -> (usize, bool) {
    binary_search_func(x, target, T::cmp)
}

/// Go's `slices.BinarySearchFunc(x, target, cmp)`.
pub fn binary_search_func<E, T: ?Sized>(
    x: &[E],
    target: &T,
    mut cmp: impl FnMut(&E, &T) -> Ordering,
) -> (usize, bool) {
    let n = x.len();
    let (mut i, mut j) = (0, n);
    while i < j {
        let h = usize::midpoint(i, j);
        if cmp(&x[h], target) == Ordering::Less {
            i = h + 1;
        } else {
            j = h;
        }
    }
    (i, i < n && cmp(&x[i], target) == Ordering::Equal)
}
