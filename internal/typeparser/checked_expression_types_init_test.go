package typeparser

import "github.com/microsoft/TypeScript/tsc/shim/checker"

func init() {
	checker.RegisterExpressionCheckedCallback(RecordCheckedExpressionType)
}
