use std::sync::atomic::Ordering;

use parking_lot::Mutex;

use crate::*;

// Rust-only: Go runs these (non-parallel) tests one after the other; libtest runs them in parallel threads, and
// they all reset the process-wide flag.
static SERIAL: Mutex<()> = Mutex::new(());

// go test: etscore/climode_test.go:5 TestIsCommandLineMode_DefaultFalse
#[test]
fn test_is_command_line_mode_default_false() {
    let _serial = SERIAL.lock();
    ets_go::testing::run(|t| {
        // Reset state for test isolation.
        climode::COMMAND_LINE_MODE.store(false, Ordering::SeqCst);

        if is_command_line_mode() {
            t.fatal("expected IsCommandLineMode() == false by default");
        }
    });
}

// go test: etscore/climode_test.go:14 TestEnterCommandLineMode_SetsTrue
#[test]
fn test_enter_command_line_mode_sets_true() {
    let _serial = SERIAL.lock();
    ets_go::testing::run(|t| {
        climode::COMMAND_LINE_MODE.store(false, Ordering::SeqCst);

        let restore = enter_command_line_mode();
        t.cleanup(restore);

        if !is_command_line_mode() {
            t.fatal("expected IsCommandLineMode() == true after EnterCommandLineMode()");
        }
    });
}

// go test: etscore/climode_test.go:25 TestEnterCommandLineMode_RestoresPrevious
#[test]
fn test_enter_command_line_mode_restores_previous() {
    let _serial = SERIAL.lock();
    ets_go::testing::run(|t| {
        climode::COMMAND_LINE_MODE.store(false, Ordering::SeqCst);

        let restore = enter_command_line_mode();
        if !is_command_line_mode() {
            t.fatal("expected true after enter");
        }

        restore();
        if is_command_line_mode() {
            t.fatal("expected false after restore");
        }
    });
}

// go test: etscore/climode_test.go:39 TestEnterCommandLineMode_Nesting
#[test]
fn test_enter_command_line_mode_nesting() {
    let _serial = SERIAL.lock();
    ets_go::testing::run(|t| {
        climode::COMMAND_LINE_MODE.store(false, Ordering::SeqCst);

        let restore1 = enter_command_line_mode();
        let restore2 = enter_command_line_mode();

        if !is_command_line_mode() {
            t.fatal("expected true after nested enter");
        }

        restore2();
        // Inner restore should keep it true since prev was already true.
        if !is_command_line_mode() {
            t.fatal("expected true after inner restore (prev was true)");
        }

        restore1();
        if is_command_line_mode() {
            t.fatal("expected false after outer restore");
        }
    });
}
