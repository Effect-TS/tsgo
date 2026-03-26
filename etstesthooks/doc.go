// Package etstesthooks provides automatic Effect package mounting for fourslash tests.
//
// This package registers a PrepareTestFSCallback that detects Effect imports in
// fourslash test files and automatically mounts the real Effect node_modules into
// the test VFS via effecttest.MountEffect.
//
// Import this package with a blank import in test files that use fourslash with Effect:
//
//	import _ "github.com/effect-ts/tsgo/etstesthooks"
package etstesthooks
