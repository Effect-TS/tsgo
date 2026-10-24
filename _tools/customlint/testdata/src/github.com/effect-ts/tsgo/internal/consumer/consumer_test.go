package consumer

import (
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

func TestRawGetter(t *testing.T) {
	(&checker.Checker{}).GetSymbolAtLocation(nil) // want "use TypeParser.GetSymbolAtLocation"
	(&checker.Checker{}).GetTypeAtLocation(nil)   // want "use TypeParser.GetTypeAtLocation"
}
