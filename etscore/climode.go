package etscore

import "sync/atomic"

var commandLineMode atomic.Bool

// EnterCommandLineMode sets the CLI mode flag to true and returns a restore
// function that reverts it to its previous value. Intended for use with defer:
//
//	restore := etscore.EnterCommandLineMode()
//	defer restore()
func EnterCommandLineMode() func() {
	prev := commandLineMode.Swap(true)
	return func() {
		commandLineMode.Store(prev)
	}
}

// IsCommandLineMode reports whether the process is currently running in CLI
// (tsc) mode, as opposed to LSP or other flows.
func IsCommandLineMode() bool {
	return commandLineMode.Load()
}
