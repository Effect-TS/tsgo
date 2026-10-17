package public

import "github.com/microsoft/TypeScript/tsc/shim/checker"

func Allowed(c *checker.Checker) { c.GetSymbolAtLocation(nil); c.GetTypeAtLocation(nil) }
