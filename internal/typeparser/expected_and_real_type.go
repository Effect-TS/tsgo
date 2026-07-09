package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// ExpectedAndRealType represents a pair of expected and actual types at an assignment site.
// This is used by diagnostic rules that need to compare what type was expected at a location
// versus what type was actually provided.
type ExpectedAndRealType struct {
	Node         *ast.Node     // The location node (for diagnostic reporting)
	ExpectedType *checker.Type // The type expected at this location
	ValueNode    *ast.Node     // The actual value node
	RealType     *checker.Type // The actual type of the value
}

// getInferredReturnType extracts the return type from a function-like declaration.
// It handles overloaded functions (multiple call signatures), type predicates,
// and regular signature return types.
func (tp *TypeParser) getInferredReturnType(declaration *ast.Node) *checker.Type {
	c := tp.checker
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
	declType := tp.GetTypeAtLocation(declaration)
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
func (tp *TypeParser) ExpectedAndRealTypes(sf *ast.SourceFile) []ExpectedAndRealType {
	if tp == nil || tp.checker == nil || sf == nil {
		return nil
	}
	c := tp.checker

	return Cached(&tp.links.ExpectedAndRealTypes, sf, func() []ExpectedAndRealType {
		var result []ExpectedAndRealType

		// Initialize BFS queue with the source file node
		queue := []*ast.Node{sf.AsNode()}
		enqueueChild := func(child *ast.Node) bool {
			queue = append(queue, child)
			return false
		}

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
						expectedType := tp.GetTypeAtLocation(nameNode)
						realType := tp.GetTypeAtLocation(vd.Initializer)
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
								realType := tp.GetTypeAtLocation(arg)
								result = append(result, ExpectedAndRealType{
									Node:         arg,
									ExpectedType: expectedType,
									ValueNode:    arg,
									RealType:     realType,
								})
							}
						}
					}
					node.ForEachChild(enqueueChild)
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
										realType := tp.GetTypeAtLocation(node)
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
				node.ForEachChild(enqueueChild)
				continue
			}

			// Pattern 4: Binary assignment (a = expr)
			if node.Kind == ast.KindBinaryExpression {
				binExpr := node.AsBinaryExpression()
				if binExpr != nil && binExpr.OperatorToken != nil &&
					binExpr.OperatorToken.Kind == ast.KindEqualsToken {
					if binExpr.Left != nil && binExpr.Right != nil {
						expectedType := tp.GetTypeAtLocation(binExpr.Left)
						realType := tp.GetTypeAtLocation(binExpr.Right)
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
						expectedType := tp.getInferredReturnType(parentDecl)
						if expectedType != nil {
							realType := tp.GetTypeAtLocation(retStmt.Expression)
							result = append(result, ExpectedAndRealType{
								Node:         node,
								ExpectedType: expectedType,
								ValueNode:    node,
								RealType:     realType,
							})
						}
					}
				}
				node.ForEachChild(enqueueChild)
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
							realType := tp.GetTypeAtLocation(body)
							result = append(result, ExpectedAndRealType{
								Node:         body,
								ExpectedType: expectedType,
								ValueNode:    body,
								RealType:     realType,
							})
						}
					} else {
						// Pattern 7: With type parameters — use inferred return type
						expectedType := tp.getInferredReturnType(node)
						if expectedType != nil {
							realType := tp.GetTypeAtLocation(body)
							result = append(result, ExpectedAndRealType{
								Node:         body,
								ExpectedType: expectedType,
								ValueNode:    body,
								RealType:     realType,
							})
						}
					}
					body.ForEachChild(enqueueChild)
					continue
				}
			}

			// Pattern 8: Satisfies expression
			if node.Kind == ast.KindSatisfiesExpression {
				satExpr := node.AsSatisfiesExpression()
				if satExpr != nil && satExpr.Expression != nil && satExpr.Type != nil {
					expectedType := tp.GetTypeAtLocation(satExpr.Type)
					realType := tp.GetTypeAtLocation(satExpr.Expression)
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
			node.ForEachChild(enqueueChild)
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
