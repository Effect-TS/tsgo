use std::sync::atomic::{AtomicBool, Ordering};

// go: etscore/climode.go:5 commandLineMode 9c21c466d0a345c1
pub(crate) static COMMAND_LINE_MODE: AtomicBool = AtomicBool::new(false);

// go: etscore/climode.go:12 EnterCommandLineMode 52009195b65cf415
// EnterCommandLineMode sets the CLI mode flag to true and returns a restore
// function that reverts it to its previous value. Intended for use with defer:
//
//	restore := etscore.EnterCommandLineMode()
//	defer restore()
pub fn enter_command_line_mode() -> impl Fn() + Send + Sync + 'static {
    let prev = COMMAND_LINE_MODE.swap(true, Ordering::SeqCst);
    move || {
        COMMAND_LINE_MODE.store(prev, Ordering::SeqCst);
    }
}

// go: etscore/climode.go:21 IsCommandLineMode 3e1ca05982b91834
// IsCommandLineMode reports whether the process is currently running in CLI
// (tsc) mode, as opposed to LSP or other flows.
pub fn is_command_line_mode() -> bool {
    COMMAND_LINE_MODE.load(Ordering::SeqCst)
}
