//! Go's `testing.T`, for ported tests.
//!
//! A Go test `func TestX(t *testing.T) { ... }` becomes
//!
//! ```ignore
//! #[test]
//! fn test_x() {
//!     ets_go::testing::run(|t| { ... });
//! }
//! ```
//!
//! and the body uses `t` as Go does:
//!
//! | Go | Rust |
//! | --- | --- |
//! | `t.Fatalf(format, args...)`, `t.Errorf`, `t.Logf`, `t.Skipf` | `t.fatal(sprintf!(format, args...))`, `t.error(..)`, `t.log(..)`, `t.skip(..)` |
//! | `t.Fatal(args...)`, `t.Error`, `t.Log`, `t.Skip` | the same with `sprintln!(args...)` (Go formats them with `Sprintln`) |
//! | `t.FailNow()`, `t.Fail()`, `t.Failed()`, `t.SkipNow()`, `t.Skipped()` | `fail_now`, `fail`, `failed`, `skip_now`, `skipped` |
//! | `t.Cleanup(f)` | `t.cleanup(f)` |
//! | `t.Run(name, f)` | `t.run(name, \|t\| ...)` (subtests run one after the other) |
//! | `t.Helper()`, `t.Parallel()` | `helper()`, `parallel()`: nothing to do (libtest already runs tests in parallel) |
//!
//! `FailNow` stops the test body as Go's `runtime.Goexit` does: by unwinding the calling thread (so, as in
//! Go, only from the test's own goroutine). Messages go to stdout, which libtest shows for a failed test.
//! When the test ends its cleanups run, last registered first; then a failed test panics (libtest's failure),
//! a skipped one prints `--- SKIP` and passes (libtest has no skipped state), and a panic in the body is
//! re-raised after the cleanups.
//!
//! Rust-only: Go's `testing.T` without `t.Context()`, async cleanups and `synctest`, which the ported effect-tsgo
//! tests do not use; plus [`on_checker_stack`] and [`lock_after_check_callback`].

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;

/// Go's `*testing.T`. Cloning shares the test, as Go code shares the pointer.
#[derive(Clone)]
pub struct T {
    inner: Arc<Common>,
}

struct Common {
    name: String,
    parent: Option<T>,
    failed: AtomicBool,
    skipped: AtomicBool,
    cleanups: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
}

/// The unwinding payload of `FailNow`.
struct FailNow;

/// The unwinding payload of `SkipNow`.
struct SkipNow;

/// Runs a test body, as Go's `tRunner` runs `func TestX(t *testing.T)`. The test is named after the libtest
/// thread (the test's path).
pub fn run(f: impl FnOnce(&T)) {
    let name = std::thread::current().name().unwrap_or("test").to_string();
    let t = T::new(name, None);
    let result = catch_unwind(AssertUnwindSafe(|| f(&t)));
    t.run_cleanups();
    t.report(result);
}

// Rust-only: the stack tsrs gives every thread that checks (`tsrs_compiler::checkerpool`, its CLI's main thread,
// its LSP workers). The checker recurses as deeply as Go's, whose goroutine stacks grow on demand, and nothing
// guards against overflow; a libtest thread has 2 MiB. Virtual memory.
pub const CHECKER_STACK_SIZE: usize = 512 << 20;

// Rust-only: runs `f` on a thread with `CHECKER_STACK_SIZE` bytes of stack, re-raising its panic. A test that
// type checks runs its body here: `on_checker_stack(|| ets_go::testing::run(|t| { .. }))`.
pub fn on_checker_stack<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
    let name = std::thread::current().name().unwrap_or("checker").to_string();
    let thread = std::thread::Builder::new()
        .name(name)
        .stack_size(CHECKER_STACK_SIZE)
        .spawn(f)
        .expect("spawn a checker thread");
    match thread.join() {
        Ok(r) => r,
        Err(payload) => resume_unwind(payload),
    }
}

// Rust-only: the checker's after-check-source-file callback (`checker.RegisterAfterCheckSourceFileCallback`, tsrs
// patch 002) is one process-wide slot, and libtest runs the tests of a binary in parallel threads of one process. A
// test that registers its own callback holds this guard from the registration until its checks are done, so no
// other such test replaces the callback in between: `let _guard = ets_go::testing::lock_after_check_callback();`.
// (Registering the Effect hooks, `ets_etscheckerhooks::init()`, always installs the same callback and needs no
// guard.) The lock does not poison: a failing test releases it for the next one.
static AFTER_CHECK_CALLBACK: Mutex<()> = Mutex::new(());

pub fn lock_after_check_callback() -> parking_lot::MutexGuard<'static, ()> {
    AFTER_CHECK_CALLBACK.lock()
}

impl T {
    fn new(name: String, parent: Option<T>) -> T {
        T {
            inner: Arc::new(Common {
                name,
                parent,
                failed: AtomicBool::new(false),
                skipped: AtomicBool::new(false),
                cleanups: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Go's `t.Name()`.
    pub fn name(&self) -> String {
        self.inner.name.clone()
    }

    /// Go's `t.Helper()`: nothing to record (messages carry no file and line).
    #[expect(clippy::unused_self, reason = "Go's t.Helper(), kept so ported calls read the same")]
    pub fn helper(&self) {}

    /// Go's `t.Parallel()`: libtest already runs tests in parallel.
    #[expect(clippy::unused_self, reason = "Go's t.Parallel(), kept so ported calls read the same")]
    pub fn parallel(&self) {}

    /// Go's `t.Log(..)` / `t.Logf(..)` with the formatted message.
    #[expect(
        clippy::unused_self,
        reason = "Go's t.Log; the output is the process's, as libtest captures it"
    )]
    pub fn log(&self, msg: impl AsRef<str>) {
        let msg = msg.as_ref();
        let msg = msg.strip_suffix('\n').unwrap_or(msg);
        let mut out = String::new();
        for (i, line) in msg.split('\n').enumerate() {
            out.push_str(if i == 0 { "    " } else { "\n        " });
            out.push_str(line);
        }
        println!("{out}");
    }

    /// Go's `t.Error(..)` / `t.Errorf(..)`: logs and marks the test failed.
    pub fn error(&self, msg: impl AsRef<str>) {
        self.log(msg);
        self.fail();
    }

    /// Go's `t.Fatal(..)` / `t.Fatalf(..)`: logs, marks the test failed and stops it.
    pub fn fatal(&self, msg: impl AsRef<str>) -> ! {
        self.log(msg);
        self.fail_now();
    }

    /// Go's `t.Skip(..)` / `t.Skipf(..)`: logs and stops the test, which counts as skipped.
    pub fn skip(&self, msg: impl AsRef<str>) -> ! {
        self.log(msg);
        self.skip_now();
    }

    /// Go's `t.Fail()`: marks the test (and its parents) failed; it keeps running.
    pub fn fail(&self) {
        if let Some(parent) = &self.inner.parent {
            parent.fail();
        }
        self.inner.failed.store(true, Ordering::SeqCst);
    }

    /// Go's `t.Failed()`.
    pub fn failed(&self) -> bool {
        self.inner.failed.load(Ordering::SeqCst)
    }

    /// Go's `t.FailNow()`: marks the test failed and stops it (unwinds the calling thread).
    pub fn fail_now(&self) -> ! {
        self.fail();
        resume_unwind(Box::new(FailNow));
    }

    /// Go's `t.SkipNow()`: marks the test skipped and stops it.
    pub fn skip_now(&self) -> ! {
        self.inner.skipped.store(true, Ordering::SeqCst);
        resume_unwind(Box::new(SkipNow));
    }

    /// Go's `t.Skipped()`.
    pub fn skipped(&self) -> bool {
        self.inner.skipped.load(Ordering::SeqCst)
    }

    /// Go's `t.Cleanup(f)`: `f` runs when the test (and its subtests) end, last registered first.
    pub fn cleanup(&self, f: impl FnOnce() + Send + 'static) {
        self.inner.cleanups.lock().push(Box::new(f));
    }

    /// Go's `t.Run(name, f)`: runs `f` as a subtest and reports whether it passed. A failed subtest fails
    /// this test; `FailNow` in it stops only the subtest.
    pub fn run(&self, name: &str, f: impl FnOnce(&T)) -> bool {
        let sub = T::new(format!("{}/{}", self.inner.name, rewrite(name)), Some(self.clone()));
        let result = catch_unwind(AssertUnwindSafe(|| f(&sub)));
        sub.run_cleanups();
        match result {
            Ok(()) => {}
            Err(payload) if payload.is::<FailNow>() || payload.is::<SkipNow>() => {}
            Err(payload) => resume_unwind(payload),
        }
        if sub.skipped() && !sub.failed() {
            println!("    --- SKIP: {}", sub.inner.name);
        }
        if sub.failed() {
            println!("    --- FAIL: {}", sub.inner.name);
        }
        !sub.failed()
    }

    /// Runs the cleanups after the test body. A cleanup may register more.
    fn run_cleanups(&self) {
        loop {
            let mut cleanups = std::mem::take(&mut *self.inner.cleanups.lock());
            if cleanups.is_empty() {
                return;
            }
            while let Some(cleanup) = cleanups.pop() {
                match catch_unwind(AssertUnwindSafe(cleanup)) {
                    Ok(()) => {}
                    Err(payload) if payload.is::<FailNow>() || payload.is::<SkipNow>() => {}
                    Err(payload) => {
                        self.fail();
                        println!("    cleanup panicked: {}", panic_message(&*payload));
                    }
                }
            }
        }
    }

    /// Ends the test: re-raises a panic of the body, fails if the test failed, notes a skip.
    fn report(&self, result: std::thread::Result<()>) {
        match result {
            Ok(()) => {}
            Err(payload) if payload.is::<FailNow>() || payload.is::<SkipNow>() => {}
            Err(payload) => resume_unwind(payload),
        }
        assert!(!self.failed(), "--- FAIL: {}", self.inner.name);
        if self.skipped() {
            eprintln!("--- SKIP: {}", self.inner.name);
        }
    }
}

/// Go's `rewrite` of a subtest name: spaces become underscores.
fn rewrite(name: &str) -> String {
    name.replace(' ', "_")
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        return (*s).to_string();
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return s.clone();
    }
    "non-string panic".to_string()
}
