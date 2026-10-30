package etscore

import "testing"

func TestEnterCommandLineModeNestingRestoresPrevious(t *testing.T) {
	commandLineMode.Store(false)

	restore1 := EnterCommandLineMode()
	restore2 := EnterCommandLineMode()

	if !IsCommandLineMode() {
		t.Fatal("expected true after nested enter")
	}

	restore2()
	if !IsCommandLineMode() {
		t.Fatal("expected true after inner restore (prev was true)")
	}

	restore1()
	if IsCommandLineMode() {
		t.Fatal("expected false after outer restore")
	}
}
