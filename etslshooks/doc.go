// Package etslshooks provides Effect code fix integration with TypeScript-Go.
//
// This package registers a CodeFixProvider that delegates to the fixables
// in internal/fixables. It should be imported (with a blank identifier) by
// the main entry point to register the Effect code fix provider with the
// TypeScript-Go language service.
//
// The package provides:
//   - EffectFixProvider: A CodeFixProvider that handles all Effect diagnostic codes
//   - Automatic delegation to internal/fixables for fix implementations
//
// Usage:
//
//	import _ "github.com/effect-ts/effect-typescript-go/etslshooks"
package etslshooks
