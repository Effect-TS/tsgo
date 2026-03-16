package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// IsGlobalErrorType reports whether the given type is exactly the global Error type.
// It performs a bidirectional assignability check to ensure the type is not a subclass
// or unrelated type. Types like any and unknown are excluded since they are
// bidirectionally assignable to everything and would produce false positives.
func IsGlobalErrorType(c *checker.Checker, t *checker.Type) bool {
	if c == nil || t == nil {
		return false
	}
	links := GetEffectLinks(c)
	return Cached(&links.IsGlobalErrorType, t, func() bool {
		// Exclude any/unknown — they are bidirectionally assignable to everything
		if t.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown) != 0 {
			return false
		}

		errorSymbol := c.ResolveName("Error", nil, ast.SymbolFlagsType, false)
		if errorSymbol == nil {
			return false
		}

		globalErrorType := c.GetDeclaredTypeOfSymbol(errorSymbol)
		if globalErrorType == nil {
			return false
		}

		return checker.Checker_isTypeAssignableTo(c, t, globalErrorType) &&
			checker.Checker_isTypeAssignableTo(c, globalErrorType, t)
	})
}
