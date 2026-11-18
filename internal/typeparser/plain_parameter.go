package typeparser

import "github.com/microsoft/TypeScript/tsc/shim/ast"

// IsPlainIdentifierParameter reports whether parameter is a parameter
// declaration that binds a single identifier, without a rest marker or a
// default value. Optional markers, type annotations and `this` parameters (whose
// name is an identifier) are accepted: callers that care check them.
func IsPlainIdentifierParameter(parameter *ast.Node) bool {
	if parameter == nil || parameter.Kind != ast.KindParameter {
		return false
	}
	declaration := parameter.AsParameterDeclaration()
	name := declaration.Name()
	return name != nil && name.Kind == ast.KindIdentifier && declaration.DotDotDotToken == nil && declaration.Initializer == nil
}
