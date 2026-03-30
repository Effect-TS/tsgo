package refactors

import (
	"github.com/effect-ts/tsgo/internal/refactor"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var MakeSchemaOpaqueWithNs = refactor.Refactor{
	Name:        "makeSchemaOpaqueWithNs",
	Description: "Make Schema opaque with namespace",
	Kind:        "rewrite.effect.makeSchemaOpaqueWithNs",
	Run:         runMakeSchemaOpaqueWithNs,
}

func runMakeSchemaOpaqueWithNs(ctx *refactor.Context) []ls.CodeAction {
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
		Description: "Make Schema opaque with namespace",
		Run: func(tracker *change.Tracker) {
			schemaId := typeparser.FindModuleIdentifier(ctx.SourceFile, "Schema")
			origName := info.identifier.AsIdentifier().Text
			newName := origName + "_"

			isEObject := info.schemaTypes.E.Flags()&checker.TypeFlagsObject != 0

			// 1. Rename the original variable identifier (append "_")
			newIdentifier := tracker.NewIdentifier(newName)
			tracker.ReplaceNode(ctx.SourceFile, info.identifier, newIdentifier, nil)

			var insertNodes []*ast.Node

			// 2. Opaque Type: export interface <Name> extends Schema.Schema.Type<typeof Name_>
			// Always uses Schema.Schema.Type, always an interface
			opaqueType := createOpaqueInterface(tracker, origName, schemaId, "Schema", "Type", newName)
			insertNodes = append(insertNodes, opaqueType)

			// 3. Build namespace members
			var nsMembers []*ast.Node

			// Encoded: export interface/type Encoded extends Schema.{Codec|Schema}.Encoded<typeof Name_>
			encodedNs := "Schema"
			if isV4 {
				encodedNs = "Codec"
			}
			var encodedMember *ast.Node
			if isEObject {
				encodedMember = createOpaqueInterface(tracker, "Encoded", schemaId, encodedNs, "Encoded", newName)
			} else {
				encodedMember = createOpaqueTypeAlias(tracker, "Encoded", schemaId, encodedNs, "Encoded", newName)
			}
			nsMembers = append(nsMembers, encodedMember)

			// Context/DecodingServices
			var contextMemberName string
			if isV4 {
				contextMemberName = "DecodingServices"
				contextMember := createOpaqueTypeAlias(tracker, "DecodingServices", schemaId, "Codec", "DecodingServices", newName)
				nsMembers = append(nsMembers, contextMember)
			} else {
				contextMemberName = "Context"
				contextMember := createOpaqueTypeAlias(tracker, "Context", schemaId, "Schema", "Context", newName)
				nsMembers = append(nsMembers, contextMember)
			}

			// V4 only: EncodingServices
			if isV4 {
				esMember := createOpaqueTypeAlias(tracker, "EncodingServices", schemaId, "Codec", "EncodingServices", newName)
				nsMembers = append(nsMembers, esMember)
			}

			// 4. Create namespace declaration
			nsBlock := tracker.NewModuleBlock(
				tracker.NewNodeList(nsMembers),
			)
			nsDecl := tracker.NewModuleDeclaration(
				opaqueExportModifiers(tracker),
				ast.KindNamespaceKeyword,
				tracker.NewIdentifier(origName),
				nsBlock,
			)
			ast.SetParentInChildren(nsDecl)
			insertNodes = append(insertNodes, nsDecl)

			// 5. Re-export const: export const <Name>: Schema.Schema<Name, Name.Encoded, Name.{Context|DecodingServices}> = Name_
			// Always uses Schema.Schema with 3 type args for the namespace variant
			typeArgs := []*ast.Node{
				tracker.NewTypeReferenceNode(tracker.NewIdentifier(origName), nil),
				tracker.NewTypeReferenceNode(
					tracker.NewQualifiedName(
						tracker.NewIdentifier(origName),
						tracker.NewIdentifier("Encoded"),
					),
					nil,
				),
				tracker.NewTypeReferenceNode(
					tracker.NewQualifiedName(
						tracker.NewIdentifier(origName),
						tracker.NewIdentifier(contextMemberName),
					),
					nil,
				),
			}

			constType := tracker.NewTypeReferenceNode(
				tracker.NewQualifiedName(
					tracker.NewIdentifier(schemaId),
					tracker.NewIdentifier("Schema"),
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
	action.Kind = "refactor.rewrite.effect.makeSchemaOpaqueWithNs"
	return []ls.CodeAction{*action}
}
