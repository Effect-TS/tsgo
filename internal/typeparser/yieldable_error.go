package typeparser

import (
	"strings"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// sourceFileProgram is a local interface for accessing program source files.
type sourceFileProgram interface {
	SourceFiles() []*ast.SourceFile
}

// YieldableErrorType returns t itself when it is assignable to Cause.YieldableError
// from the "effect" package, or nil otherwise. Returns nil for never and any types.
func (tp *TypeParser) YieldableErrorType(t *checker.Type) *checker.Type {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}
	return Cached(&tp.links.YieldableErrorType, t, func() *checker.Type {
		// never is assignable to everything, so we need to exclude it
		if t.Flags()&checker.TypeFlagsNever != 0 {
			return nil
		}
		// any is assignable to everything, so we need to exclude it
		if t.Flags()&checker.TypeFlagsAny != 0 {
			return nil
		}

		prog, ok := tp.program.(sourceFileProgram)
		if !ok || prog == nil {
			return nil
		}

		for _, sf := range prog.SourceFiles() {
			if sf == nil {
				continue
			}

			// Check this source file belongs to the "effect" package
			pkg := tp.PackageJsonForSourceFile(sf)
			if pkg == nil {
				continue
			}
			name, ok := pkg.Name.GetValue()
			if !ok || !strings.EqualFold(name, "effect") {
				continue
			}

			moduleSym := checker.Checker_getSymbolOfDeclaration(tp.checker, sf.AsNode())
			if moduleSym == nil {
				continue
			}

			// Look for the YieldableError export
			exportSym := tp.checker.TryGetMemberInModuleExportsAndProperties("YieldableError", moduleSym)
			if exportSym == nil {
				continue
			}

			// Verify this is the Cause module by checking for a Cause export
			causeSym := tp.checker.TryGetMemberInModuleExportsAndProperties("Cause", moduleSym)
			if causeSym == nil {
				continue
			}

			exportSym = tp.resolveAliasedSymbol(exportSym)
			if exportSym == nil {
				continue
			}

			yieldableErrorType := tp.checker.GetDeclaredTypeOfSymbol(exportSym)
			if yieldableErrorType == nil {
				continue
			}

			if checker.Checker_isTypeAssignableTo(tp.checker, t, yieldableErrorType) {
				return t
			}
		}

		return nil
	})
}
