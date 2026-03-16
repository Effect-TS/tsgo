package typeparser

import (
	"github.com/effect-ts/effect-typescript-go/internal/checkerutils"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// getInferredReturnType extracts the return type from a function-like declaration.
// It handles overloaded functions (multiple call signatures), type predicates,
// and regular signature return types.
func getInferredReturnType(c *checker.Checker, declaration *ast.Node) *checker.Type {
	if declaration == nil {
		return nil
	}

	// Check that the declaration has a body
	bodyData := declaration.BodyData()
	if bodyData == nil || bodyData.Body == nil {
		return nil
	}

	var returnType *checker.Type

	// Try overloaded function handling:
	// Get the type at the declaration location, get call signatures,
	// and if there are multiple signatures, union their return types.
	declType := checkerutils.GetTypeAtLocation(c, declaration)
	if declType != nil {
		signatures := c.GetSignaturesOfType(declType, checker.SignatureKindCall)
		if len(signatures) > 1 {
			var returnTypes []*checker.Type
			for _, sig := range signatures {
				rt := c.GetReturnTypeOfSignature(sig)
				if rt != nil {
					returnTypes = append(returnTypes, rt)
				}
			}
			if len(returnTypes) > 0 {
				returnType = c.GetUnionType(returnTypes)
			}
		}
	}

	if returnType == nil {
		sig := c.GetSignatureFromDeclaration(declaration)
		if sig != nil {
			typePredicate := c.GetTypePredicateOfSignature(sig)
			if typePredicate != nil && typePredicate.Type() != nil {
				return typePredicate.Type()
			}
			returnType = c.GetReturnTypeOfSignature(sig)
		}
	}

	return returnType
}

// ExpectedAndRealTypes walks the AST of a source file using breadth-first traversal
// and collects pairs of expected vs actual types at assignment sites.
//
// It recognizes 8 assignment site patterns:
// 1. Variable declaration with initializer (const a: T = expr)
// 2. Call expression arguments (fn(a))
// 3. Object literal property keys ({ key: expr } as { key: T })
// 4. Binary assignment (a = expr)
// 5. Return statement (return expr)
// 6. Arrow function body without type params ((): T => expr)
// 7. Arrow function body with type params (<A>(): T => expr)
// 8. Satisfies expression (expr satisfies T)
func ExpectedAndRealTypes(c *checker.Checker, sf *ast.SourceFile) []ExpectedAndRealType {
	if c == nil || sf == nil {
		return nil
	}

	links := GetEffectLinks(c)
	return Cached(&links.ExpectedAndRealTypes, sf, func() []ExpectedAndRealType {
		var result []ExpectedAndRealType

		// Initialize BFS queue with the source file node
		queue := []*ast.Node{sf.AsNode()}

		for len(queue) > 0 {
			// Dequeue from front (FIFO/breadth-first) to match TypeScript's shift() behavior
			node := queue[0]
			queue = queue[1:]

			if node == nil {
				continue
			}

			// Pattern 1: Variable declaration with initializer
			if node.Kind == ast.KindVariableDeclaration {
				vd := node.AsVariableDeclaration()
				if vd != nil && vd.Initializer != nil {
					nameNode := vd.Name()
					if nameNode != nil {
						expectedType := checkerutils.GetTypeAtLocation(c, nameNode)
						realType := checkerutils.GetTypeAtLocation(c, vd.Initializer)
						result = append(result, ExpectedAndRealType{
							Node:         nameNode,
							ExpectedType: expectedType,
							ValueNode:    vd.Initializer,
							RealType:     realType,
						})
					}
					queue = append(queue, vd.Initializer)
					continue
				}
			}

			// Pattern 2: Call expression arguments
			if node.Kind == ast.KindCallExpression {
				call := node.AsCallExpression()
				if call != nil {
					resolvedSig := c.GetResolvedSignature(node)
					if resolvedSig != nil {
						params := resolvedSig.Parameters()
						if call.Arguments != nil {
							for i, param := range params {
								if i >= len(call.Arguments.Nodes) {
									break
								}
								arg := call.Arguments.Nodes[i]
								if arg == nil {
									continue
								}
								expectedType := c.GetTypeOfSymbolAtLocation(param, node)
								realType := checkerutils.GetTypeAtLocation(c, arg)
								result = append(result, ExpectedAndRealType{
									Node:         arg,
									ExpectedType: expectedType,
									ValueNode:    arg,
									RealType:     realType,
								})
							}
						}
					}
					for child := range node.IterChildren() {
						queue = append(queue, child)
					}
					continue
				}
			}

			// Pattern 3: Object literal property keys
			if node.Kind == ast.KindIdentifier || node.Kind == ast.KindStringLiteral ||
				node.Kind == ast.KindNumericLiteral || node.Kind == ast.KindNoSubstitutionTemplateLiteral {
				parent := node.Parent
				if parent != nil && ast.IsObjectLiteralElement(parent) {
					grandparent := parent.Parent
					if grandparent != nil && grandparent.Kind == ast.KindObjectLiteralExpression {
						// Check that this node is the name of the property (not the value)
						nameNode := getObjectLiteralElementName(parent)
						if nameNode == node {
							contextualType := c.GetContextualType(grandparent, checker.ContextFlagsNone)
							if contextualType != nil {
								name := getNodeTextForPropertyLookup(node)
								if name != "" {
									sym := c.GetPropertyOfType(contextualType, name)
									if sym != nil {
										expectedType := c.GetTypeOfSymbolAtLocation(sym, node)
										realType := checkerutils.GetTypeAtLocation(c, node)
										result = append(result, ExpectedAndRealType{
											Node:         node,
											ExpectedType: expectedType,
											ValueNode:    node,
											RealType:     realType,
										})
									}
								}
							}
						}
					}
				}
				for child := range node.IterChildren() {
					queue = append(queue, child)
				}
				continue
			}

			// Pattern 4: Binary assignment (a = expr)
			if node.Kind == ast.KindBinaryExpression {
				binExpr := node.AsBinaryExpression()
				if binExpr != nil && binExpr.OperatorToken != nil &&
					binExpr.OperatorToken.Kind == ast.KindEqualsToken {
					if binExpr.Left != nil && binExpr.Right != nil {
						expectedType := checkerutils.GetTypeAtLocation(c, binExpr.Left)
						realType := checkerutils.GetTypeAtLocation(c, binExpr.Right)
						result = append(result, ExpectedAndRealType{
							Node:         binExpr.Left,
							ExpectedType: expectedType,
							ValueNode:    binExpr.Right,
							RealType:     realType,
						})
					}
					if binExpr.Right != nil {
						queue = append(queue, binExpr.Right)
					}
					continue
				}
			}

			// Pattern 5: Return statement
			if node.Kind == ast.KindReturnStatement {
				retStmt := node.AsReturnStatement()
				if retStmt != nil && retStmt.Expression != nil {
					parentDecl := ast.GetContainingFunction(node)
					if parentDecl != nil {
						expectedType := getInferredReturnType(c, parentDecl)
						if expectedType != nil {
							realType := checkerutils.GetTypeAtLocation(c, retStmt.Expression)
							result = append(result, ExpectedAndRealType{
								Node:         node,
								ExpectedType: expectedType,
								ValueNode:    node,
								RealType:     realType,
							})
						}
					}
				}
				for child := range node.IterChildren() {
					queue = append(queue, child)
				}
				continue
			}

			// Pattern 6 & 7: Arrow function body (expression body)
			if node.Kind == ast.KindArrowFunction {
				fn := node.AsArrowFunction()
				if fn != nil && fn.Body != nil && fn.Body.Kind != ast.KindBlock {
					body := fn.Body
					hasTypeParams := fn.TypeParameters != nil && len(fn.TypeParameters.Nodes) > 0

					if !hasTypeParams {
						// Pattern 6: No type parameters — use contextual type
						expectedType := c.GetContextualType(body, checker.ContextFlagsNone)
						if expectedType != nil {
							realType := checkerutils.GetTypeAtLocation(c, body)
							result = append(result, ExpectedAndRealType{
								Node:         body,
								ExpectedType: expectedType,
								ValueNode:    body,
								RealType:     realType,
							})
						}
					} else {
						// Pattern 7: With type parameters — use inferred return type
						expectedType := getInferredReturnType(c, node)
						if expectedType != nil {
							realType := checkerutils.GetTypeAtLocation(c, body)
							result = append(result, ExpectedAndRealType{
								Node:         body,
								ExpectedType: expectedType,
								ValueNode:    body,
								RealType:     realType,
							})
						}
					}
					for child := range body.IterChildren() {
						queue = append(queue, child)
					}
					continue
				}
			}

			// Pattern 8: Satisfies expression
			if node.Kind == ast.KindSatisfiesExpression {
				satExpr := node.AsSatisfiesExpression()
				if satExpr != nil && satExpr.Expression != nil && satExpr.Type != nil {
					expectedType := checkerutils.GetTypeAtLocation(c, satExpr.Type)
					realType := checkerutils.GetTypeAtLocation(c, satExpr.Expression)
					result = append(result, ExpectedAndRealType{
						Node:         satExpr.Expression,
						ExpectedType: expectedType,
						ValueNode:    satExpr.Expression,
						RealType:     realType,
					})
					queue = append(queue, satExpr.Expression)
					continue
				}
			}

			// No pattern matched — queue all children for traversal
			for child := range node.IterChildren() {
				queue = append(queue, child)
			}
		}

		return result
	})
}

// getObjectLiteralElementName returns the name node of an object literal element
// (PropertyAssignment, ShorthandPropertyAssignment, etc.), or nil.
func getObjectLiteralElementName(node *ast.Node) *ast.Node {
	if node == nil {
		return nil
	}
	switch node.Kind {
	case ast.KindPropertyAssignment:
		pa := node.AsPropertyAssignment()
		if pa != nil {
			return pa.Name()
		}
	case ast.KindShorthandPropertyAssignment:
		spa := node.AsShorthandPropertyAssignment()
		if spa != nil {
			return spa.Name()
		}
	}
	return nil
}

// getNodeTextForPropertyLookup extracts the text from an identifier or string literal
// node for use in property type lookup.
func getNodeTextForPropertyLookup(node *ast.Node) string {
	if node == nil {
		return ""
	}
	switch node.Kind {
	case ast.KindIdentifier:
		return scanner.GetTextOfNode(node)
	case ast.KindStringLiteral:
		return node.AsStringLiteral().Text
	}
	return ""
}
