package etscore

import "testing"

func TestIsCommandLineMode_DefaultFalse(t *testing.T) {
	// Reset state for test isolation.
	commandLineMode.Store(false)

	if IsCommandLineMode() {
		t.Fatal("expected IsCommandLineMode() == false by default")
	}
}

func TestEnterCommandLineMode_SetsTrue(t *testing.T) {
	commandLineMode.Store(false)

	restore := EnterCommandLineMode()
	defer restore()

	if !IsCommandLineMode() {
		t.Fatal("expected IsCommandLineMode() == true after EnterCommandLineMode()")
	}
}

func TestEnterCommandLineMode_RestoresPrevious(t *testing.T) {
	commandLineMode.Store(false)

	restore := EnterCommandLineMode()
	if !IsCommandLineMode() {
		t.Fatal("expected true after enter")
	}

	restore()
	if IsCommandLineMode() {
		t.Fatal("expected false after restore")
	}
}

func TestEnterCommandLineMode_Nesting(t *testing.T) {
	commandLineMode.Store(false)

	restore1 := EnterCommandLineMode()
	restore2 := EnterCommandLineMode()

	if !IsCommandLineMode() {
		t.Fatal("expected true after nested enter")
	}

	restore2()
	// Inner restore should keep it true since prev was already true.
	if !IsCommandLineMode() {
		t.Fatal("expected true after inner restore (prev was true)")
	}

	restore1()
	if IsCommandLineMode() {
		t.Fatal("expected false after outer restore")
	}
}
