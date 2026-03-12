package refactors

import (
	"github.com/effect-ts/effect-typescript-go/internal/checkerutils"
	"github.com/effect-ts/effect-typescript-go/internal/effectutil"
	"github.com/effect-ts/effect-typescript-go/internal/refactor"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var MakeSchemaOpaque = refactor.Refactor{
	Name:        "makeSchemaOpaque",
	Description: "Make Schema opaque",
	Kind:        "rewrite.effect.makeSchemaOpaque",
	Run:         runMakeSchemaOpaque,
}

// schemaVarInfo holds the result of finding a schema variable declaration.
type schemaVarInfo struct {
	identifier  *ast.Node // the variable name identifier
	varStatement *ast.Node // the enclosing VariableStatement
	varDeclList *ast.Node // the VariableDeclarationList
	initializer *ast.Node // the initializer expression
	schemaTypes *typeparser.SchemaTypes
}

// findSchemaVariableDeclaration walks the ancestor chain from the cursor to find a
// VariableDeclaration whose initializer has a Schema type. Returns nil if not found.
func findSchemaVariableDeclaration(ctx *refactor.Context, c *checker.Checker) *schemaVarInfo {
	if ctx.Span.Pos() == ctx.Span.End() {
		return nil
	}

	token := astnav.GetTokenAtPosition(ctx.SourceFile, ctx.Span.Pos())
	if token == nil {
		return nil
	}

	for node := token; node != nil && node.Kind != ast.KindSourceFile; node = node.Parent {
		if node.Kind != ast.KindVariableDeclaration {
			continue
		}

		varDecl := node.AsVariableDeclaration()

		// Must have an identifier name
		nameNode := node.Name()
		if nameNode == nil || nameNode.Kind != ast.KindIdentifier {
			continue
		}

		// Name must be within the selection span
		namePos := astnav.GetStartOfNode(nameNode, ctx.SourceFile, false)
		nameEnd := nameNode.End()
		if ctx.Span.Pos() < namePos || ctx.Span.Pos() >= nameEnd {
			continue
		}

		// Must have an initializer
		if varDecl.Initializer == nil {
			continue
		}

		// Parent chain: VariableDeclaration → VariableDeclarationList → VariableStatement
		varDeclList := node.Parent
		if varDeclList == nil || varDeclList.Kind != ast.KindVariableDeclarationList {
			continue
		}
		varStatement := varDeclList.Parent
		if varStatement == nil || varStatement.Kind != ast.KindVariableStatement {
			continue
		}

		// Check the initializer's type is a Schema type
		initType := checkerutils.GetTypeAtLocation(c, varDecl.Initializer)
		if initType == nil {
			continue
		}

		schemaTypes := typeparser.EffectSchemaTypes(c, initType, varDecl.Initializer)
		if schemaTypes == nil {
			continue
		}

		return &schemaVarInfo{
			identifier:   nameNode,
			varStatement: varStatement,
			varDeclList:  varDeclList,
			initializer:  varDecl.Initializer,
			schemaTypes:  schemaTypes,
		}
	}

	return nil
}

// createTypeofReference creates: typeof <name>
func createTypeofReference(tracker *change.Tracker, name string) *ast.Node {
	return tracker.NewTypeQueryNode(
		tracker.NewIdentifier(name),
		nil,
	)
}

// createSchemaPropertyAccess creates: <schemaId>.<ns>.<member> as a property access expression.
// Used in ExpressionWithTypeArguments for heritage clauses.
func createSchemaPropertyAccess(tracker *change.Tracker, schemaId string, ns string, member string) *ast.Node {
	return tracker.NewPropertyAccessExpression(
		tracker.NewPropertyAccessExpression(
			tracker.NewIdentifier(schemaId),
			nil,
			tracker.NewIdentifier(ns),
			ast.NodeFlagsNone,
		),
		nil,
		tracker.NewIdentifier(member),
		ast.NodeFlagsNone,
	)
}

// createSchemaQualifiedName creates: <schemaId>.<ns>.<member> as a qualified name.
// Used in TypeReferenceNode for type alias declarations.
func createSchemaQualifiedName(tracker *change.Tracker, schemaId string, ns string, member string) *ast.Node {
	return tracker.NewQualifiedName(
		tracker.NewQualifiedName(
			tracker.NewIdentifier(schemaId),
			tracker.NewIdentifier(ns),
		),
		tracker.NewIdentifier(member),
	)
}

// createHeritageTypeRef creates: <schemaId>.<ns>.<member><typeof <inferFrom>>
// as an ExpressionWithTypeArguments node for use in interface heritage clauses.
func createHeritageTypeRef(tracker *change.Tracker, schemaId string, ns string, member string, inferFrom string) *ast.Node {
	return tracker.NewExpressionWithTypeArguments(
		createSchemaPropertyAccess(tracker, schemaId, ns, member),
		tracker.NewNodeList([]*ast.Node{
			createTypeofReference(tracker, inferFrom),
		}),
	)
}

// createTypeAliasTypeRef creates: <schemaId>.<ns>.<member><typeof <inferFrom>>
// as a TypeReferenceNode for use in type alias declarations.
func createTypeAliasTypeRef(tracker *change.Tracker, schemaId string, ns string, member string, inferFrom string) *ast.Node {
	return tracker.NewTypeReferenceNode(
		createSchemaQualifiedName(tracker, schemaId, ns, member),
		tracker.NewNodeList([]*ast.Node{
			createTypeofReference(tracker, inferFrom),
		}),
	)
}

func opaqueExportModifiers(tracker *change.Tracker) *ast.ModifierList {
	return tracker.NewModifierList([]*ast.Node{
		tracker.NewModifier(ast.KindExportKeyword),
	})
}

// createOpaqueInterface creates: export interface <name> extends <heritage> { }
func createOpaqueInterface(tracker *change.Tracker, name string, schemaId string, ns string, member string, inferFrom string) *ast.Node {
	heritage := createHeritageTypeRef(tracker, schemaId, ns, member, inferFrom)
	node := tracker.NewInterfaceDeclaration(
		opaqueExportModifiers(tracker),
		tracker.NewIdentifier(name),
		nil, // typeParameters
		tracker.NewNodeList([]*ast.Node{
			tracker.NewHeritageClause(
				ast.KindExtendsKeyword,
				tracker.NewNodeList([]*ast.Node{heritage}),
			),
		}),
		tracker.NewNodeList([]*ast.Node{}), // empty members
	)
	ast.SetParentInChildren(node)
	return node
}

// createOpaqueTypeAlias creates: export type <name> = <schemaId>.<ns>.<member><typeof inferFrom>
func createOpaqueTypeAlias(tracker *change.Tracker, name string, schemaId string, ns string, member string, inferFrom string) *ast.Node {
	typeRef := createTypeAliasTypeRef(tracker, schemaId, ns, member, inferFrom)
	node := tracker.NewTypeAliasDeclaration(
		opaqueExportModifiers(tracker),
		tracker.NewIdentifier(name),
		nil, // typeParameters
		typeRef,
	)
	ast.SetParentInChildren(node)
	return node
}

// createOpaqueTypeDecl creates an interface (if isObject) or type alias for the given schema property.
func createOpaqueTypeDecl(tracker *change.Tracker, name string, schemaId string, ns string, member string, inferFrom string, isObject bool) *ast.Node {
	if isObject {
		return createOpaqueInterface(tracker, name, schemaId, ns, member, inferFrom)
	}
	return createOpaqueTypeAlias(tracker, name, schemaId, ns, member, inferFrom)
}

func runMakeSchemaOpaque(ctx *refactor.Context) []ls.CodeAction {
	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	info := findSchemaVariableDeclaration(ctx, c)
	if info == nil {
		return nil
	}

	version := typeparser.SupportedEffectVersion(c)
	isV4 := version == typeparser.EffectMajorV4

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Make Schema opaque",
		Run: func(tracker *change.Tracker) {
			schemaId := effectutil.FindModuleIdentifier(ctx.SourceFile, "Schema")
			origName := info.identifier.AsIdentifier().Text
			newName := origName + "_"

			isAObject := info.schemaTypes.A.Flags()&checker.TypeFlagsObject != 0
			isEObject := info.schemaTypes.E.Flags()&checker.TypeFlagsObject != 0

			// 1. Rename the original variable identifier (append "_")
			newIdentifier := tracker.NewIdentifier(newName)
			tracker.ReplaceNode(ctx.SourceFile, info.identifier, newIdentifier, nil)

			// Build all nodes to insert after the variable statement
			var insertNodes []*ast.Node

			// 2. Opaque Type: export interface/type <Name> extends Schema.Schema.Type<typeof Name_>
			opaqueType := createOpaqueTypeDecl(tracker, origName, schemaId, "Schema", "Type", newName, isAObject)
			insertNodes = append(insertNodes, opaqueType)

			// 3. Encoded: export interface/type <Name>Encoded extends Schema.{Codec|Schema}.Encoded<typeof Name_>
			encodedNs := "Schema"
			if isV4 {
				encodedNs = "Codec"
			}
			encodedName := origName + "Encoded"
			encodedType := createOpaqueTypeDecl(tracker, encodedName, schemaId, encodedNs, "Encoded", newName, isEObject)
			insertNodes = append(insertNodes, encodedType)

			// 4. Context / DecodingServices
			contextNs := "Schema"
			contextMember := "Context"
			contextName := origName + "Context"
			if isV4 {
				contextNs = "Codec"
				contextMember = "DecodingServices"
				contextName = origName + "DecodingServices"
			}
			contextType := createOpaqueTypeAlias(tracker, contextName, schemaId, contextNs, contextMember, newName)
			insertNodes = append(insertNodes, contextType)

			// 5. V4 only: EncodingServices
			var encodingServicesName string
			if isV4 {
				encodingServicesName = origName + "EncodingServices"
				esType := createOpaqueTypeAlias(tracker, encodingServicesName, schemaId, "Codec", "EncodingServices", newName)
				insertNodes = append(insertNodes, esType)
			}

			// 6. Re-export const: export const <Name>: Schema.{Codec|Schema}<...> = Name_
			schemaTypeNs := "Schema"
			if isV4 {
				schemaTypeNs = "Codec"
			}
			typeArgs := []*ast.Node{
				tracker.NewTypeReferenceNode(tracker.NewIdentifier(origName), nil),
				tracker.NewTypeReferenceNode(tracker.NewIdentifier(encodedName), nil),
				tracker.NewTypeReferenceNode(tracker.NewIdentifier(contextName), nil),
			}
			if isV4 {
				typeArgs = append(typeArgs, tracker.NewTypeReferenceNode(tracker.NewIdentifier(encodingServicesName), nil))
			}

			constType := tracker.NewTypeReferenceNode(
				tracker.NewQualifiedName(
					tracker.NewIdentifier(schemaId),
					tracker.NewIdentifier(schemaTypeNs),
				),
				tracker.NewNodeList(typeArgs),
			)

			constDecl := tracker.NewVariableDeclaration(
				tracker.NewIdentifier(origName),
				nil,       // exclamationToken
				constType, // type annotation
				tracker.NewIdentifier(newName),
			)
			constDeclList := tracker.NewVariableDeclarationList(
				ast.NodeFlagsConst,
				tracker.NewNodeList([]*ast.Node{constDecl}),
			)
			constStatement := tracker.NewVariableStatement(
				opaqueExportModifiers(tracker),
				constDeclList,
			)
			ast.SetParentInChildren(constStatement)
			insertNodes = append(insertNodes, constStatement)

			// Insert all nodes after the variable statement
			tracker.InsertNodesAfter(ctx.SourceFile, info.varStatement, insertNodes)
		},
	})
	if action == nil {
		return nil
	}
	action.Kind = "refactor.rewrite.effect.makeSchemaOpaque"
	return []ls.CodeAction{*action}
}
