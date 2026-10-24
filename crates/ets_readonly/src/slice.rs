// go: internal/readonly/slice.go:11 Slice
// Slice preserves the backing array and nil state of a borrowed slice.
// Elements are returned by value; pointers inside elements remain borrowed.
//
// Rust: the view borrows the backing slice (`&'a [T]`), so the owner cannot mutate it while the view lives; a nil Go
// slice is `None`. The zero value (`var s readonly.Slice[T]`) is `Slice::default()`.
pub struct Slice<'a, T> {
    items: Option<&'a [T]>,
}

// go: internal/readonly/slice.go:11 Slice
impl<T> Clone for Slice<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

// go: internal/readonly/slice.go:11 Slice
impl<T> Copy for Slice<'_, T> {}

// go: internal/readonly/slice.go:11 Slice
impl<T> Default for Slice<'_, T> {
    fn default() -> Self {
        Slice { items: None }
    }
}

// go: internal/readonly/slice.go:16 New
// New borrows items. Its owner must not mutate published elements.
//
// Rust: `items` is a `&[T]` (a non-nil Go slice) or an `Option<&[T]>` (`None` is a nil Go slice).
pub fn new<'a, T>(items: impl Into<Option<&'a [T]>>) -> Slice<'a, T> {
    Slice { items: items.into() }
}

impl<'a, T> Slice<'a, T> {
    // go: internal/readonly/slice.go:18 Slice.Len
    #[expect(clippy::len_without_is_empty, reason = "Go's Slice has Len and no IsEmpty")]
    pub fn len(&self) -> i64 {
        self.backing().len() as i64
    }

    // go: internal/readonly/slice.go:20 Slice.At
    // Rust: a shared reference into the backing slice stands for Go's copy of the element: neither can write the
    // element slot.
    pub fn at(&self, index: i64) -> &'a T {
        &self.backing()[index as usize]
    }

    // go: internal/readonly/slice.go:22 Slice.Slice
    #[must_use]
    pub fn slice(&self, start: i64, end: i64) -> Slice<'a, T> {
        // Go: slicing a nil slice with in-bounds indices (only 0:0) gives nil again.
        let sliced = &self.backing()[start as usize..end as usize];
        new(self.items.map(|_| sliced))
    }

    // go: internal/readonly/slice.go:24 Slice.IsNil
    pub fn is_nil(&self) -> bool {
        self.items.is_none()
    }

    // go: internal/readonly/slice.go:26 Slice.All
    pub fn all(&self) -> impl Iterator<Item = (i64, &'a T)> + 'a {
        self.backing().iter().enumerate().map(|(index, value)| (index as i64, value))
    }

    // Rust-only: the elements of the view, empty for nil (Go reads `s.items` directly).
    fn backing(&self) -> &'a [T] {
        self.items.unwrap_or_default()
    }
}

impl<T: Clone> Slice<'_, T> {
    // go: internal/readonly/slice.go:29 Slice.Copy
    // Copy returns a caller-owned slice, preserving nil versus empty.
    pub fn copy(&self) -> Option<Vec<T>> {
        self.items.map(<[T]>::to_vec)
    }

    // go: internal/readonly/slice.go:32 Slice.AppendTo
    // AppendTo copies elements into the caller's destination.
    pub fn append_to(&self, mut dst: Vec<T>) -> Vec<T> {
        dst.extend_from_slice(self.backing());
        dst
    }
}
