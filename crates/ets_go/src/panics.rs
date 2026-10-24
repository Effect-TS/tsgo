//! Go's panic semantics for a whole process: a panic that no `recover` stops ends the program.
//!
//! A Rust panic ends only the thread or the tokio task it happens on, so a process that must die as Go's does
//! installs [`install_fatal_hook`] (first thing in `main`): the panic hook
//! prints the panic and aborts, unless the panicking thread is inside [`recover`], the counterpart of a Go
//! `defer func() { recover() }()` boundary. Code that ports a Go `recover` uses [`recover`] instead of
//! `std::panic::catch_unwind`, so the hook knows the panic will be caught.
//!
//! `panic = "abort"` is not an option: it would make the recover boundaries abort too.

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, UnwindSafe};
use std::sync::Once;

thread_local! {
    // How many `recover` scopes the current thread is inside.
    static RECOVERING: Cell<u32> = const { Cell::new(0) };
}

/// Go's `defer func() { if r := recover(); r != nil { .. } }()` around `f`: `std::panic::catch_unwind`, and
/// a panic inside it does not end the process under [`install_fatal_hook`].
pub fn recover<R>(f: impl FnOnce() -> R + UnwindSafe) -> std::thread::Result<R> {
    RECOVERING.with(|depth| depth.set(depth.get() + 1));
    let result = std::panic::catch_unwind(AssertUnwindSafe(f));
    RECOVERING.with(|depth| depth.set(depth.get() - 1));
    result
}

/// Whether the current thread is inside a [`recover`] scope.
pub fn recovering() -> bool {
    RECOVERING.with(Cell::get) > 0
}

/// Makes an unrecovered panic fatal, as in Go: after the default hook prints it, the process aborts, unless
/// the panicking thread is inside [`recover`]. Installed once; later calls do nothing.
pub fn install_fatal_hook() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            default_hook(info);
            if !recovering() {
                std::process::abort();
            }
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recover_tracks_its_scope() {
        assert!(!recovering());
        let inside = recover(recovering);
        assert_eq!(inside.ok(), Some(true));
        assert!(!recovering());
        let caught = recover(|| -> () { panic!("boom") });
        assert!(caught.is_err());
        assert!(!recovering());
        let nested = recover(|| recover(recovering).ok() == Some(true) && recovering());
        assert_eq!(nested.ok(), Some(true));
    }
}
