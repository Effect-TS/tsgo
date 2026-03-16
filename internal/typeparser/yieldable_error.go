package typeparser

import (
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// sourceFileProgram is a local interface for accessing program source files.
type sourceFileProgram interface {
	SourceFiles() []*ast.SourceFile
}

// IsYieldableErrorType reports whether the given type is assignable to Cause.YieldableError
// from the "effect" package. Returns false for never and any types.
func IsYieldableErrorType(c *checker.Checker, t *checker.Type) bool {
	if c == nil || t == nil {
		return false
	}
	links := GetEffectLinks(c)
	return Cached(&links.IsYieldableErrorType, t, func() bool {
		// never is assignable to everything, so we need to exclude it
		if t.Flags()&checker.TypeFlagsNever != 0 {
			return false
		}
		// any is assignable to everything, so we need to exclude it
		if t.Flags()&checker.TypeFlagsAny != 0 {
			return false
		}

		prog, ok := c.Program().(sourceFileProgram)
		if !ok || prog == nil {
			return false
		}

		for _, sf := range prog.SourceFiles() {
			if sf == nil {
				continue
			}

			// Check this source file belongs to the "effect" package
			pkg := PackageJsonForSourceFile(c, sf)
			if pkg == nil {
				continue
			}
			name, ok := pkg.Name.GetValue()
			if !ok || !strings.EqualFold(name, "effect") {
				continue
			}

			moduleSym := moduleSymbolFromSourceFile(c, sf)
			if moduleSym == nil {
				continue
			}

			// Look for the YieldableError export
			exportSym := c.TryGetMemberInModuleExportsAndProperties("YieldableError", moduleSym)
			if exportSym == nil {
				continue
			}

			// Verify this is the Cause module by checking for a Cause export
			causeSym := c.TryGetMemberInModuleExportsAndProperties("Cause", moduleSym)
			if causeSym == nil {
				continue
			}

			exportSym = resolveAliasedSymbol(c, exportSym)
			if exportSym == nil {
				continue
			}

			yieldableErrorType := c.GetDeclaredTypeOfSymbol(exportSym)
			if yieldableErrorType == nil {
				continue
			}

			if checker.Checker_isTypeAssignableTo(c, t, yieldableErrorType) {
				return true
			}
		}

		return false
	})
}
