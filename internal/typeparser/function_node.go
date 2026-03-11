package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
)

// GetFunctionLikeName returns the name string from a function-like node.
// Handles FunctionDeclaration, FunctionExpression, MethodDeclaration (with identifier name),
// and falls back to checking the parent VariableDeclaration for arrow/expression functions.
func GetFunctionLikeName(node *ast.Node) string {
	switch node.Kind {
	case ast.KindFunctionDeclaration:
		fd := node.AsFunctionDeclaration()
		if fd.Name() != nil {
			return fd.Name().AsIdentifier().Text
		}
	case ast.KindFunctionExpression:
		fe := node.AsFunctionExpression()
		if fe.Name() != nil {
			return fe.Name().AsIdentifier().Text
		}
	case ast.KindMethodDeclaration:
		md := node.AsMethodDeclaration()
		if md.Name() != nil && md.Name().Kind == ast.KindIdentifier {
			return md.Name().AsIdentifier().Text
		}
	}

	// Check parent variable declaration for arrow/expression functions
	if node.Parent != nil && node.Parent.Kind == ast.KindVariableDeclaration {
		vd := node.Parent.AsVariableDeclaration()
		if vd.Name() != nil && vd.Name().Kind == ast.KindIdentifier {
			return vd.Name().AsIdentifier().Text
		}
	}

	return ""
}

// GetFunctionLikeBody returns the body node from a function-like node.
// Handles FunctionDeclaration, FunctionExpression, ArrowFunction, and MethodDeclaration.
func GetFunctionLikeBody(node *ast.Node) *ast.Node {
	switch node.Kind {
	case ast.KindFunctionDeclaration:
		fd := node.AsFunctionDeclaration()
		if fd.Body != nil {
			return fd.Body.AsNode()
		}
	case ast.KindFunctionExpression:
		fe := node.AsFunctionExpression()
		if fe.Body != nil {
			return fe.Body.AsNode()
		}
	case ast.KindArrowFunction:
		af := node.AsArrowFunction()
		if af.Body != nil {
			return af.Body
		}
	case ast.KindMethodDeclaration:
		md := node.AsMethodDeclaration()
		if md.Body != nil {
			return md.Body.AsNode()
		}
	}
	return nil
}

// GetFunctionLikeTypeParameters returns the type parameters NodeList from a function-like node.
// Handles ArrowFunction, FunctionExpression, FunctionDeclaration, and MethodDeclaration.
func GetFunctionLikeTypeParameters(node *ast.Node) *ast.NodeList {
	switch node.Kind {
	case ast.KindArrowFunction:
		return node.AsArrowFunction().TypeParameters
	case ast.KindFunctionExpression:
		return node.AsFunctionExpression().TypeParameters
	case ast.KindFunctionDeclaration:
		return node.AsFunctionDeclaration().TypeParameters
	case ast.KindMethodDeclaration:
		return node.AsMethodDeclaration().TypeParameters
	}
	return nil
}

// GetFunctionLikeParameters returns the parameters NodeList from a function-like node.
// Handles ArrowFunction, FunctionExpression, FunctionDeclaration, and MethodDeclaration.
func GetFunctionLikeParameters(node *ast.Node) *ast.NodeList {
	switch node.Kind {
	case ast.KindArrowFunction:
		return node.AsArrowFunction().Parameters
	case ast.KindFunctionExpression:
		return node.AsFunctionExpression().Parameters
	case ast.KindFunctionDeclaration:
		return node.AsFunctionDeclaration().Parameters
	case ast.KindMethodDeclaration:
		return node.AsMethodDeclaration().Parameters
	}
	return nil
}
