// Package etsexecutehooks provides command-line filtering for Effect diagnostics.
//
// This package registers a FilterDiagnosticsForExitCodeCallback that filters out
// Effect diagnostics from exit-code and noEmitOnError determination based on the
// IgnoreEffectSuggestionsInTscExitCode and IgnoreEffectWarningsInTscExitCode
// configuration options.
//
// Import this package with a blank import in the main entry point:
//
//	import _ "github.com/effect-ts/tsgo/etsexecutehooks"
package etsexecutehooks
