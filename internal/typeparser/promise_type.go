package typeparser

import (
	"github.com/microsoft/typescript-go/shim/checker"
)

// PromiseType returns t when it is the global Promise type or a reference to it.
// It intentionally does not match arbitrary thenables or PromiseLike values.
func (tp *TypeParser) PromiseType(t *checker.Type) *checker.Type {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}

	return Cached(&tp.links.PromiseType, t, func() *checker.Type {
		getGlobalPromiseTypeChecked := checker.Checker_getGlobalPromiseTypeChecked(tp.checker)
		if getGlobalPromiseTypeChecked == nil {
			return nil
		}

		globalPromiseType := getGlobalPromiseTypeChecked()
		if globalPromiseType == nil || globalPromiseType == checker.Checker_emptyGenericType(tp.checker) {
			return nil
		}

		if checker.Checker_isReferenceToType(tp.checker, t, globalPromiseType) || t == globalPromiseType {
			return t
		}

		return nil
	})
}
