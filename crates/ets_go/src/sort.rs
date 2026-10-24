//! Go's `sort` package: `sort.Slice`, `sort.Sort`, `sort.Strings`, `sort.SliceStable`, `sort.Search`.
//!
//! Go's unstable sorts are pattern-defeating quicksort (`sort/zsortfunc.go`, `slices/zsortanyfunc.go`,
//! `slices/zsortordered.go`, all generated from one template by `sort/gen_sort_variants.go`). They are
//! deterministic, and the order they leave equal elements in differs from Rust's `sort_unstable_by`, so a
//! port whose ties can reach observable output must use these. The template is ported once, over
//! [`LessSwap`]; `slices::sort_func` and friends are in [`crate::slices`].
//!
//! Stable sorts have exactly one correct result, so `sort.SliceStable` / `sort.Stable` map to Rust's stable
//! `sort_by`.

use std::cmp::Ordering;

/// The `Less` + `Swap` pair the pdqsort template works on (Go's `lessSwap` / `sort.Interface` minus `Len`).
pub trait LessSwap {
    fn less(&mut self, i: usize, j: usize) -> bool;
    fn swap(&mut self, i: usize, j: usize);
}

/// Go's `sort.Interface`.
pub trait Interface {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn less(&self, i: usize, j: usize) -> bool;
    fn swap(&mut self, i: usize, j: usize);
}

struct InterfaceLessSwap<'a, T: Interface + ?Sized>(&'a mut T);

impl<T: Interface + ?Sized> LessSwap for InterfaceLessSwap<'_, T> {
    fn less(&mut self, i: usize, j: usize) -> bool {
        self.0.less(i, j)
    }

    fn swap(&mut self, i: usize, j: usize) {
        self.0.swap(i, j);
    }
}

/// A slice and a less function on its elements: what `sort.Slice`'s `less(i, j)` closure reads.
pub(crate) struct SliceLess<'a, T, F> {
    pub(crate) data: &'a mut [T],
    pub(crate) less: F,
}

impl<T, F: FnMut(&T, &T) -> bool> LessSwap for SliceLess<'_, T, F> {
    fn less(&mut self, i: usize, j: usize) -> bool {
        (self.less)(&self.data[i], &self.data[j])
    }

    fn swap(&mut self, i: usize, j: usize) {
        self.data.swap(i, j);
    }
}

/// Go's `sort.Sort(data)`.
pub fn sort<T: Interface + ?Sized>(data: &mut T) {
    let n = data.len();
    if n <= 1 {
        return;
    }
    let limit = bits_len(n);
    pdqsort(&mut InterfaceLessSwap(data), 0, n, limit);
}

/// Go's `sort.Slice(x, func(i, j int) bool { return less(x[i], x[j]) })`: Go's pdqsort, so equal elements
/// end up where Go puts them.
pub fn slice<T>(x: &mut [T], less: impl FnMut(&T, &T) -> bool) {
    let length = x.len();
    let limit = bits_len(length);
    pdqsort(&mut SliceLess { data: x, less }, 0, length, limit);
}

/// Go's `sort.SliceStable`. A stable sort has one correct result, so this is Rust's stable sort.
pub fn slice_stable<T>(x: &mut [T], mut less: impl FnMut(&T, &T) -> bool) {
    x.sort_by(|a, b| {
        if less(a, b) {
            Ordering::Less
        } else if less(b, a) {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    });
}

/// Go's `sort.Strings` (which is `slices.Sort`).
pub fn strings(x: &mut [String]) {
    crate::slices::sort(x);
}

/// Go's `sort.Ints` (which is `slices.Sort`).
pub fn ints(x: &mut [i64]) {
    crate::slices::sort(x);
}

/// Go's `sort.Search(n, f)`: the smallest index in `[0, n)` at which `f` is true, or `n`.
pub fn search(n: usize, mut f: impl FnMut(usize) -> bool) -> usize {
    let (mut i, mut j) = (0, n);
    while i < j {
        let h = usize::midpoint(i, j);
        if !f(h) {
            i = h + 1;
        } else {
            j = h;
        }
    }
    i
}

/// Go's `bits.Len(uint(n))`.
pub(crate) fn bits_len(n: usize) -> usize {
    n.bit_width() as usize
}

// The pdqsort template (sort/zsortfunc.go), one function per Go function.

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortedHint {
    Unknown,
    Increasing,
    Decreasing,
}

struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn next_power_of_two(length: usize) -> usize {
    1 << bits_len(length)
}

fn insertion_sort<D: LessSwap + ?Sized>(data: &mut D, a: usize, b: usize) {
    for i in a + 1..b {
        let mut j = i;
        while j > a && data.less(j, j - 1) {
            data.swap(j, j - 1);
            j -= 1;
        }
    }
}

fn sift_down<D: LessSwap + ?Sized>(data: &mut D, lo: usize, hi: usize, first: usize) {
    let mut root = lo;
    loop {
        let mut child = 2 * root + 1;
        if child >= hi {
            break;
        }
        if child + 1 < hi && data.less(first + child, first + child + 1) {
            child += 1;
        }
        if !data.less(first + root, first + child) {
            return;
        }
        data.swap(first + root, first + child);
        root = child;
    }
}

fn heap_sort<D: LessSwap + ?Sized>(data: &mut D, a: usize, b: usize) {
    let first = a;
    let lo = 0;
    let hi = b - a;
    for i in (0..=(hi - 1) / 2).rev() {
        sift_down(data, i, hi, first);
    }
    for i in (0..hi).rev() {
        data.swap(first, first + i);
        sift_down(data, lo, i, first);
    }
}

pub(crate) fn pdqsort<D: LessSwap + ?Sized>(
    data: &mut D,
    mut a: usize,
    mut b: usize,
    mut limit: usize,
) {
    const MAX_INSERTION: usize = 12;
    let mut was_balanced = true;
    let mut was_partitioned = true;
    loop {
        let length = b - a;
        if length <= MAX_INSERTION {
            insertion_sort(data, a, b);
            return;
        }
        // Fall back to heapsort if too many bad choices were made.
        if limit == 0 {
            heap_sort(data, a, b);
            return;
        }
        // If the last partitioning was imbalanced, we need to breakPatterns.
        if !was_balanced {
            break_patterns(data, a, b);
            limit -= 1;
        }
        let (mut pivot, mut hint) = choose_pivot(data, a, b);
        if hint == SortedHint::Decreasing {
            reverse_range(data, a, b);
            // The chosen pivot was pivot-a elements after the start of the array.
            // After reversing it is pivot-a elements before the end of the array.
            pivot = (b - 1) - (pivot - a);
            hint = SortedHint::Increasing;
        }
        // The slice is likely already sorted.
        if was_balanced && was_partitioned && hint == SortedHint::Increasing {
            if partial_insertion_sort(data, a, b) {
                return;
            }
        }
        // Probably the slice contains many duplicate elements, partition the slice into
        // elements equal to and elements greater than the pivot.
        if a > 0 && !data.less(a - 1, pivot) {
            let mid = partition_equal(data, a, b, pivot);
            a = mid;
            continue;
        }
        let (mid, already_partitioned) = partition(data, a, b, pivot);
        was_partitioned = already_partitioned;
        let (left_len, right_len) = (mid - a, b - mid);
        let balance_threshold = length / 8;
        if left_len < right_len {
            was_balanced = left_len >= balance_threshold;
            pdqsort(data, a, mid, limit);
            a = mid + 1;
        } else {
            was_balanced = right_len >= balance_threshold;
            pdqsort(data, mid + 1, b, limit);
            b = mid;
        }
    }
}

fn partition<D: LessSwap + ?Sized>(
    data: &mut D,
    a: usize,
    b: usize,
    pivot: usize,
) -> (usize, bool) {
    data.swap(a, pivot);
    let (mut i, mut j) = (a + 1, b - 1); // i and j are inclusive of the elements remaining to be partitioned
    while i <= j && data.less(i, a) {
        i += 1;
    }
    while i <= j && !data.less(j, a) {
        j -= 1;
    }
    if i > j {
        data.swap(j, a);
        return (j, true);
    }
    data.swap(i, j);
    i += 1;
    j -= 1;
    loop {
        while i <= j && data.less(i, a) {
            i += 1;
        }
        while i <= j && !data.less(j, a) {
            j -= 1;
        }
        if i > j {
            break;
        }
        data.swap(i, j);
        i += 1;
        j -= 1;
    }
    data.swap(j, a);
    (j, false)
}

fn partition_equal<D: LessSwap + ?Sized>(data: &mut D, a: usize, b: usize, pivot: usize) -> usize {
    data.swap(a, pivot);
    let (mut i, mut j) = (a + 1, b - 1); // i and j are inclusive of the elements remaining to be partitioned
    loop {
        while i <= j && !data.less(a, i) {
            i += 1;
        }
        while i <= j && data.less(a, j) {
            j -= 1;
        }
        if i > j {
            break;
        }
        data.swap(i, j);
        i += 1;
        j -= 1;
    }
    i
}

fn partial_insertion_sort<D: LessSwap + ?Sized>(data: &mut D, a: usize, b: usize) -> bool {
    const MAX_STEPS: usize = 5; // maximum number of adjacent out-of-order pairs that will get shifted
    const SHORTEST_SHIFTING: usize = 50; // don't shift any elements on short arrays
    let mut i = a + 1;
    for _ in 0..MAX_STEPS {
        while i < b && !data.less(i, i - 1) {
            i += 1;
        }
        if i == b {
            return true;
        }
        if b - a < SHORTEST_SHIFTING {
            return false;
        }
        data.swap(i, i - 1);
        // Shift the smaller one to the left.
        if i - a >= 2 {
            let mut j = i - 1;
            while j >= 1 {
                if !data.less(j, j - 1) {
                    break;
                }
                data.swap(j, j - 1);
                j -= 1;
            }
        }
        // Shift the greater one to the right.
        if b - i >= 2 {
            for j in i + 1..b {
                if !data.less(j, j - 1) {
                    break;
                }
                data.swap(j, j - 1);
            }
        }
    }
    false
}

fn break_patterns<D: LessSwap + ?Sized>(data: &mut D, a: usize, b: usize) {
    let length = b - a;
    if length >= 8 {
        let mut random = Xorshift(length as u64);
        let modulus = next_power_of_two(length);
        for idx in a + (length / 4) * 2 - 1..=a + (length / 4) * 2 + 1 {
            let mut other = (random.next() as usize) & (modulus - 1);
            if other >= length {
                other -= length;
            }
            data.swap(idx, a + other);
        }
    }
}

fn choose_pivot<D: LessSwap + ?Sized>(data: &mut D, a: usize, b: usize) -> (usize, SortedHint) {
    const SHORTEST_NINTHER: usize = 50;
    const MAX_SWAPS: usize = 4 * 3;
    let l = b - a;
    let mut swaps = 0;
    let mut i = a + l / 4;
    let mut j = a + l / 4 * 2;
    let mut k = a + l / 4 * 3;
    if l >= 8 {
        if l >= SHORTEST_NINTHER {
            // Tukey ninther method, the idea came from Rust's implementation.
            i = median_adjacent(data, i, &mut swaps);
            j = median_adjacent(data, j, &mut swaps);
            k = median_adjacent(data, k, &mut swaps);
        }
        // Find the median among i, j, k and stores it into j.
        j = median(data, i, j, k, &mut swaps);
    }
    match swaps {
        0 => (j, SortedHint::Increasing),
        MAX_SWAPS => (j, SortedHint::Decreasing),
        _ => (j, SortedHint::Unknown),
    }
}

fn order2<D: LessSwap + ?Sized>(
    data: &mut D,
    a: usize,
    b: usize,
    swaps: &mut usize,
) -> (usize, usize) {
    if data.less(b, a) {
        *swaps += 1;
        return (b, a);
    }
    (a, b)
}

fn median<D: LessSwap + ?Sized>(
    data: &mut D,
    a: usize,
    b: usize,
    c: usize,
    swaps: &mut usize,
) -> usize {
    let (a, b) = order2(data, a, b, swaps);
    let (b, c) = order2(data, b, c, swaps);
    let (_a, b) = order2(data, a, b, swaps);
    let _ = c;
    b
}

fn median_adjacent<D: LessSwap + ?Sized>(data: &mut D, a: usize, swaps: &mut usize) -> usize {
    median(data, a - 1, a, a + 1, swaps)
}

fn reverse_range<D: LessSwap + ?Sized>(data: &mut D, a: usize, b: usize) {
    let mut i = a;
    let mut j = b - 1;
    while i < j {
        data.swap(i, j);
        i += 1;
        j -= 1;
    }
}
