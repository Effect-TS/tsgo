use std::sync::atomic::Ordering;

use crate::*;

// go test: etscore/climode_test.go:5 TestEnterCommandLineModeNestingRestoresPrevious 58bb0559539ac799
#[test]
fn test_enter_command_line_mode_nesting_restores_previous() {
    ets_go::testing::run(|t| {
        climode::COMMAND_LINE_MODE.store(false, Ordering::SeqCst);

        let restore1 = enter_command_line_mode();
        let restore2 = enter_command_line_mode();

        if !is_command_line_mode() {
            t.fatal("expected true after nested enter");
        }

        restore2();
        if !is_command_line_mode() {
            t.fatal("expected true after inner restore (prev was true)");
        }

        restore1();
        if is_command_line_mode() {
            t.fatal("expected false after outer restore");
        }
    });
}
