// effect-tsgo's tests are not tracked by the manifest (porting/scope.toml), so these carry `// go test:` notes
// instead of origin markers.

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

use ets_go::sprintf;
use ets_readonly as readonly;

// Rust-only: the values behind cells, so the test can mutate the backing array while a view borrows it, as the Go
// test does through its own slice header.
fn values(cells: &[Cell<i64>]) -> Vec<i64> {
    cells.iter().map(Cell::get).collect()
}

// go test: internal/readonly/slice_test.go:10 TestSliceOwnershipAndIteration
#[test]
fn test_slice_ownership_and_iteration() {
    ets_go::testing::run(|t| {
        t.parallel();
        let backing = [Cell::new(2), Cell::new(3), Cell::new(5)];
        let view = readonly::new(&backing[..]);
        backing[1].set(7);
        let got = values(&view.slice(1, 3).copy().unwrap_or_default());
        if got != [7, 5] {
            t.fatal(sprintf!("borrowed subslice = %v, want [7 5]", got));
        }
        let owned = view.copy().unwrap_or_default();
        owned[0].set(99);
        let appended = view.append_to(vec![Cell::new(1)]);
        appended[2].set(99);
        let mut values = Vec::new();
        let mut indexes = Vec::new();
        for (index, value) in view.all() {
            indexes.push(index);
            values.push(value.get());
        }
        if values != [2, 7, 5] || indexes != [0, 1, 2] {
            t.fatal(sprintf!("iteration returned %v at %v", values, indexes));
        }
        let mut visits = 0;
        #[expect(clippy::never_loop, reason = "the Go test breaks out of the first iteration")]
        for _ in view.all() {
            visits += 1;
            break;
        }
        if visits != 1 {
            t.fatal(sprintf!("early stop visited %d elements", visits));
        }
    });
}

// go test: internal/readonly/slice_test.go:40 TestSliceNilAndBounds
#[test]
fn test_slice_nil_and_bounds() {
    ets_go::testing::run(|t| {
        t.parallel();
        let absent = readonly::Slice::<i64>::default();
        let empty = readonly::new::<i64>(&[][..]);
        if absent.len() != 0
            || !absent.is_nil()
            || absent.copy().is_some()
            || !absent.slice(0, 0).is_nil()
        {
            t.fatal("zero view lost its nil state");
        }
        if empty.len() != 0
            || empty.is_nil()
            || empty.copy().is_none()
            || empty.slice(0, 0).is_nil()
        {
            t.fatal("empty view became nil");
        }
        let one = [1_i64];
        let operations: [Box<dyn Fn()>; 4] = [
            Box::new(|| {
                absent.at(0);
            }),
            Box::new(|| {
                let _ = empty.slice(0, 1);
            }),
            Box::new(|| {
                readonly::new(&one[..]).at(-1);
            }),
            Box::new(|| {
                let _ = readonly::new(&one[..]).slice(1, 0);
            }),
        ];
        for operation in operations {
            if catch_unwind(AssertUnwindSafe(operation)).is_ok() {
                t.error("out-of-bounds access did not panic");
            }
        }
    });
}
