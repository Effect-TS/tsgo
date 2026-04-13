// Package schemagen converts TypeScript AST type nodes into Effect Schema API
// call expressions. It is used by the typeToEffectSchema and typeToEffectSchemaClass
// refactors to generate Schema.Struct, Schema.Class, and related schema declarations
// from interface and type alias declarations.
package schemagen

import (
	"errors"
	"fmt"

	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/ls/change"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// Errors returned by schema generation when a type cannot be converted.
var (
	ErrTypeParametersNotSupported       = errors.New("could not process types with type parameters")
	ErrOnlyLiteralPropertiesSupported   = errors.New("only literal properties are supported")
	ErrRequiredExplicitTypes            = errors.New("only explicit types are supported")
	ErrIndexSignatureMultipleParameters = errors.New("only index signatures with one parameter are supported")
)

// SchemaGen holds the context needed to convert AST type nodes into Schema expressions.
type SchemaGen struct {
	Tracker          *change.Tracker
	SourceFile       *ast.SourceFile
	SchemaIdentifier string
	Version          typeparser.EffectMajorVersion
}

// New creates a SchemaGen from the given tracker and source file, resolving the
// Schema import identifier automatically.
func New(tracker *change.Tracker, sf *ast.SourceFile, version typeparser.EffectMajorVersion) *SchemaGen {
	return &SchemaGen{
		Tracker:          tracker,
		SourceFile:       sf,
		SchemaIdentifier: typeparser.FindModuleIdentifier(sf, "Schema"),
		Version:          version,
	}
}

// createApiPropertyAccess creates Schema.<name> (e.g., Schema.String).
func (g *SchemaGen) createApiPropertyAccess(name string) *ast.Node {
	return g.Tracker.NewPropertyAccessExpression(
		g.Tracker.NewIdentifier(g.SchemaIdentifier),
		nil,
		g.Tracker.NewIdentifier(name),
		ast.NodeFlagsNone,
	)
}

// createApiCall creates Schema.<name>(...args) (e.g., Schema.Array(Schema.String)).
func (g *SchemaGen) createApiCall(name string, args []*ast.Node) *ast.Node {
	return g.Tracker.NewCallExpression(
		g.Tracker.NewPropertyAccessExpression(
			g.Tracker.NewIdentifier(g.SchemaIdentifier),
			nil,
			g.Tracker.NewIdentifier(name),
			ast.NodeFlagsNone,
		),
		nil, // questionDotToken
		nil, // typeArguments
		g.Tracker.NewNodeList(args),
		ast.NodeFlagsNone,
	)
}

// unsupportedComment creates a placeholder identifier for unsupported conversions.
// This produces /* Not supported conversion: <text> */ in the emitted output.
func (g *SchemaGen) unsupportedComment(node *ast.Node) *ast.Node {
	text := scanner.GetTextOfNode(node)
	return g.Tracker.NewIdentifier(fmt.Sprintf("undefined /* Not supported conversion: %s */", text))
}

// processNode recursively converts a TypeScript type AST node into a Schema expression.
func (g *SchemaGen) processNode(node *ast.Node) (*ast.Node, error) {
	if node == nil {
		return g.createApiPropertyAccess("Unknown"), nil
	}

	// Primitive keywords
	switch node.Kind {
	case ast.KindAnyKeyword:
		return g.createApiPropertyAccess("Any"), nil
	case ast.KindNeverKeyword:
		return g.createApiPropertyAccess("Never"), nil
	case ast.KindUnknownKeyword:
		return g.createApiPropertyAccess("Unknown"), nil
	case ast.KindVoidKeyword:
		return g.createApiPropertyAccess("Void"), nil
	case ast.KindNullKeyword:
		return g.createApiPropertyAccess("Null"), nil
	case ast.KindUndefinedKeyword:
		return g.createApiPropertyAccess("Undefined"), nil
	case ast.KindStringKeyword:
		return g.createApiPropertyAccess("String"), nil
	case ast.KindNumberKeyword:
		return g.createApiPropertyAccess("Number"), nil
	case ast.KindBooleanKeyword:
		return g.createApiPropertyAccess("Boolean"), nil
	case ast.KindBigIntKeyword:
		return g.createApiPropertyAccess("BigInt"), nil
	}

	// Literal types: string/number/boolean literals and null
	if node.Kind == ast.KindLiteralType {
		lt := node.AsLiteralTypeNode()
		if lt.Literal.Kind == ast.KindNullKeyword {
			return g.createApiPropertyAccess("Null"), nil
		}
		literals, ok := g.parseAllLiterals(node)
		if ok && len(literals) > 0 {
			return g.createApiCall("Literal", literals), nil
		}
	}

	// Union types: A | B
	if node.Kind == ast.KindUnionType {
		ut := node.AsUnionTypeNode()
		if ut.Types == nil {
			return g.unsupportedComment(node), nil
		}

		// Optimization: try to collapse all-literal unions into Schema.Literal(...)
		if g.Version != typeparser.EffectMajorV4 {
			allLiterals, ok := g.parseAllLiterals(node)
			if ok && len(allLiterals) > 0 {
				return g.createApiCall("Literal", allLiterals), nil
			}
		}

		members, err := g.processNodeList(ut.Types.Nodes)
		if err != nil {
			return nil, err
		}

		var args []*ast.Node
		if g.Version == typeparser.EffectMajorV4 {
			args = []*ast.Node{g.Tracker.NewArrayLiteralExpression(g.Tracker.NewNodeList(members), false)}
		} else {
			args = members
		}
		return g.createApiCall("Union", args), nil
	}

	// Intersection types: A & B
	if node.Kind == ast.KindIntersectionType {
		it := node.AsIntersectionTypeNode()
		if it.Types == nil || len(it.Types.Nodes) == 0 {
			return g.unsupportedComment(node), nil
		}

		members, err := g.processNodeList(it.Types.Nodes)
		if err != nil {
			return nil, err
		}

		if len(members) == 1 {
			return members[0], nil
		}

		// firstSchema.pipe(Schema.extend(second), Schema.extend(third), ...)
		first := members[0]
		extendArgs := make([]*ast.Node, len(members)-1)
		for i := 1; i < len(members); i++ {
			extendArgs[i-1] = g.createApiCall("extend", []*ast.Node{members[i]})
		}
		return g.Tracker.NewCallExpression(
			g.Tracker.NewPropertyAccessExpression(
				first, nil, g.Tracker.NewIdentifier("pipe"), ast.NodeFlagsNone,
			),
			nil, nil,
			g.Tracker.NewNodeList(extendArgs),
			ast.NodeFlagsNone,
		), nil
	}

	// Type operator: keyof T, readonly T
	if node.Kind == ast.KindTypeOperator {
		to := node.AsTypeOperatorNode()
		if to.Operator == ast.KindKeyOfKeyword {
			inner, err := g.processNode(to.Type)
			if err != nil {
				return nil, err
			}
			return g.createApiCall("keyof", []*ast.Node{inner}), nil
		}
		if to.Operator == ast.KindReadonlyKeyword {
			return g.processNode(to.Type)
		}
	}

	// Array type: T[]
	if node.Kind == ast.KindArrayType {
		at := node.AsArrayTypeNode()
		elemSchema, err := g.processNode(at.ElementType)
		if err != nil {
			return nil, err
		}
		return g.createApiCall("Array", []*ast.Node{elemSchema}), nil
	}

	// Tuple type: [A, B, C]
	if node.Kind == ast.KindTupleType {
		tt := node.AsTupleTypeNode()
		var elems []*ast.Node
		if tt.Elements != nil {
			var err error
			elems, err = g.processNodeList(tt.Elements.Nodes)
			if err != nil {
				return nil, err
			}
		}
		var args []*ast.Node
		if g.Version == typeparser.EffectMajorV4 {
			args = []*ast.Node{g.Tracker.NewArrayLiteralExpression(g.Tracker.NewNodeList(elems), false)}
		} else {
			args = elems
		}
		return g.createApiCall("Tuple", args), nil
	}

	// Type literal (object shape): { a: string, b: number }
	if node.Kind == ast.KindTypeLiteral {
		tl := node.AsTypeLiteralNode()
		var memberNodes []*ast.Node
		if tl.Members != nil {
			memberNodes = tl.Members.Nodes
		}
		properties, records, err := g.processMembers(memberNodes)
		if err != nil {
			return nil, err
		}
		args := []*ast.Node{
			g.Tracker.NewObjectLiteralExpression(g.Tracker.NewNodeList(properties), true),
		}
		args = append(args, records...)
		return g.createApiCall("Struct", args), nil
	}

	// Parenthesized type: (A)
	if node.Kind == ast.KindParenthesizedType {
		pt := node.AsParenthesizedTypeNode()
		return g.processNode(pt.Type)
	}

	// Type reference: resolve known Effect types and data types
	if node.Kind == ast.KindTypeReference {
		tr := node.AsTypeReferenceNode()
		dataTypeName := g.entityNameToDataTypeName(tr.TypeName)
		if dataTypeName != "" {
			return g.processTypeReference(node, dataTypeName)
		}

		// Simple type reference without type arguments → pass through as identifier
		if tr.TypeArguments == nil || len(tr.TypeArguments.Nodes) == 0 {
			return g.typeEntityNameToNode(tr.TypeName), nil
		}
	}

	// Unsupported
	return g.unsupportedComment(node), nil
}

// processTypeReference handles known type references like Array, Record, Option, etc.
func (g *SchemaGen) processTypeReference(node *ast.Node, dataTypeName string) (*ast.Node, error) {
	tr := node.AsTypeReferenceNode()
	var typeArgs []*ast.Node
	if tr.TypeArguments != nil {
		typeArgs = tr.TypeArguments.Nodes
	}

	switch dataTypeName {
	case "Duration", "Date":
		return g.createApiPropertyAccess(dataTypeName), nil

	case "Option", "Chunk", "Array":
		elements, err := g.processNodeList(typeArgs)
		if err != nil {
			return nil, err
		}
		return g.createApiCall(dataTypeName, elements), nil

	case "Record":
		elements, err := g.processNodeList(typeArgs)
		if err != nil {
			return nil, err
		}
		if len(elements) >= 2 {
			if g.Version == typeparser.EffectMajorV4 {
				return g.createApiCall("Record", []*ast.Node{elements[0], elements[1]}), nil
			}
			return g.createApiCall(dataTypeName, []*ast.Node{
				g.Tracker.NewObjectLiteralExpression(g.Tracker.NewNodeList([]*ast.Node{
					g.Tracker.NewPropertyAssignment(nil, g.Tracker.NewIdentifier("key"), nil, nil, elements[0]),
					g.Tracker.NewPropertyAssignment(nil, g.Tracker.NewIdentifier("value"), nil, nil, elements[1]),
				}), false),
			}), nil
		}
		return g.unsupportedComment(node), nil

	case "Either":
		elements, err := g.processNodeList(typeArgs)
		if err != nil {
			return nil, err
		}
		if len(elements) >= 2 {
			return g.createApiCall(dataTypeName, []*ast.Node{
				g.Tracker.NewObjectLiteralExpression(g.Tracker.NewNodeList([]*ast.Node{
					g.Tracker.NewPropertyAssignment(nil, g.Tracker.NewIdentifier("right"), nil, nil, elements[0]),
					g.Tracker.NewPropertyAssignment(nil, g.Tracker.NewIdentifier("left"), nil, nil, elements[1]),
				}), false),
			}), nil
		}
		return g.unsupportedComment(node), nil

	case "Pick", "Omit":
		if len(typeArgs) != 2 {
			return g.unsupportedComment(node), nil
		}
		baseType, err := g.processNode(typeArgs[0])
		if err != nil {
			return nil, err
		}
		stringLiterals, ok := g.parseAllLiterals(typeArgs[1])
		if !ok || len(stringLiterals) == 0 {
			return g.unsupportedComment(node), nil
		}
		// Use lowercase: "pick" or "omit"
		methodName := "pick"
		if dataTypeName == "Omit" {
			methodName = "omit"
		}
		return g.Tracker.NewCallExpression(
			g.Tracker.NewPropertyAccessExpression(
				baseType, nil, g.Tracker.NewIdentifier("pipe"), ast.NodeFlagsNone,
			),
			nil, nil,
			g.Tracker.NewNodeList([]*ast.Node{
				g.createApiCall(methodName, stringLiterals),
			}),
			ast.NodeFlagsNone,
		), nil
	}

	return g.unsupportedComment(node), nil
}

// entityNameToDataTypeName maps a type name to a recognized data type name.
func (g *SchemaGen) entityNameToDataTypeName(typeName *ast.Node) string {
	if typeName == nil {
		return ""
	}
	if typeName.Kind == ast.KindIdentifier {
		name := typeName.AsIdentifier().Text
		switch name {
		case "Date", "Pick", "Omit", "Record":
			return name
		case "ReadonlyArray", "Array":
			return "Array"
		}
		return ""
	}
	// Qualified name: Module.Name (e.g., Option.Option)
	if typeName.Kind == ast.KindQualifiedName {
		qn := typeName.AsQualifiedName()
		if qn.Left == nil || qn.Right == nil {
			return ""
		}
		if qn.Left.Kind != ast.KindIdentifier {
			return ""
		}
		rightName := scanner.GetTextOfNode(qn.Right)
		leftName := scanner.GetTextOfNode(qn.Left)
		// Check if leftName is an imported module identifier for a known module
		for _, moduleName := range []string{"Option", "Either", "Chunk", "Duration"} {
			imported := typeparser.FindModuleIdentifier(g.SourceFile, moduleName)
			if leftName == imported && rightName == moduleName {
				return moduleName
			}
		}
	}
	return ""
}

// typeEntityNameToNode converts a type entity name to an expression node (identifier or property access).
func (g *SchemaGen) typeEntityNameToNode(entityName *ast.Node) *ast.Node {
	if entityName == nil {
		return g.Tracker.NewIdentifier("unknown")
	}
	if entityName.Kind == ast.KindIdentifier {
		return g.Tracker.NewIdentifier(entityName.AsIdentifier().Text)
	}
	if entityName.Kind == ast.KindQualifiedName {
		qn := entityName.AsQualifiedName()
		left := g.typeEntityNameToNode(qn.Left)
		return g.Tracker.NewPropertyAccessExpression(
			left, nil,
			g.Tracker.NewIdentifier(scanner.GetTextOfNode(qn.Right)),
			ast.NodeFlagsNone,
		)
	}
	return g.Tracker.NewIdentifier(scanner.GetTextOfNode(entityName))
}

// parseAllLiterals tries to parse a type node as a collection of literal values.
// Returns (literals, true) on success, (nil, false) if the node contains non-literal types.
func (g *SchemaGen) parseAllLiterals(node *ast.Node) ([]*ast.Node, bool) {
	if node == nil {
		return nil, false
	}

	if node.Kind == ast.KindLiteralType {
		lt := node.AsLiteralTypeNode()
		switch lt.Literal.Kind {
		case ast.KindStringLiteral:
			return []*ast.Node{g.Tracker.NewStringLiteral(lt.Literal.AsStringLiteral().Text, 0)}, true
		case ast.KindNumericLiteral:
			return []*ast.Node{g.Tracker.NewNumericLiteral(lt.Literal.AsNumericLiteral().Text, 0)}, true
		case ast.KindTrueKeyword:
			return []*ast.Node{g.Tracker.NewKeywordExpression(ast.KindTrueKeyword)}, true
		case ast.KindFalseKeyword:
			return []*ast.Node{g.Tracker.NewKeywordExpression(ast.KindFalseKeyword)}, true
		}
		return nil, false
	}

	if node.Kind == ast.KindUnionType {
		ut := node.AsUnionTypeNode()
		if ut.Types == nil {
			return nil, false
		}
		var result []*ast.Node
		for _, t := range ut.Types.Nodes {
			lits, ok := g.parseAllLiterals(t)
			if !ok {
				return nil, false
			}
			result = append(result, lits...)
		}
		return result, true
	}

	if node.Kind == ast.KindParenthesizedType {
		pt := node.AsParenthesizedTypeNode()
		return g.parseAllLiterals(pt.Type)
	}

	return nil, false
}

// processNodeList processes a list of type nodes, converting each to a Schema expression.
func (g *SchemaGen) processNodeList(nodes []*ast.Node) ([]*ast.Node, error) {
	result := make([]*ast.Node, 0, len(nodes))
	for _, n := range nodes {
		expr, err := g.processNode(n)
		if err != nil {
			return nil, err
		}
		result = append(result, expr)
	}
	return result, nil
}

// membersResult holds the processed properties and records from type members.
type membersResult struct {
	Properties []*ast.Node
	Records    []*ast.Node
}

// processMembers handles property signatures and index signatures from type members.
func (g *SchemaGen) processMembers(members []*ast.Node) (properties []*ast.Node, records []*ast.Node, err error) {
	for _, member := range members {
		if member.Kind == ast.KindPropertySignature {
			ps := member.AsPropertySignatureDeclaration()
			name := member.Name()
			if name == nil {
				return nil, nil, ErrOnlyLiteralPropertiesSupported
			}
			if name.Kind != ast.KindIdentifier && name.Kind != ast.KindStringLiteral {
				return nil, nil, ErrOnlyLiteralPropertiesSupported
			}
			if ps.Type == nil {
				return nil, nil, ErrRequiredExplicitTypes
			}

			propSchema, err := g.processNode(ps.Type)
			if err != nil {
				return nil, nil, err
			}

			// Wrap in Schema.optional(...) if question token present
			if member.QuestionToken() != nil {
				propSchema = g.createApiCall("optional", []*ast.Node{propSchema})
			}

			// Create property name node
			var propName *ast.Node
			if name.Kind == ast.KindIdentifier {
				propName = g.Tracker.NewIdentifier(name.AsIdentifier().Text)
			} else {
				propName = g.Tracker.NewStringLiteral(name.AsStringLiteral().Text, 0)
			}

			assignment := g.Tracker.NewPropertyAssignment(nil, propName, nil, nil, propSchema)
			properties = append(properties, assignment)
		}

		if member.Kind == ast.KindIndexSignature {
			is := member.AsIndexSignatureDeclaration()
			if is.Parameters == nil || len(is.Parameters.Nodes) != 1 {
				return nil, nil, ErrIndexSignatureMultipleParameters
			}
			param := is.Parameters.Nodes[0]
			paramDecl := param.AsParameterDeclaration()
			if paramDecl.Type == nil {
				return nil, nil, ErrRequiredExplicitTypes
			}

			key, err := g.processNode(paramDecl.Type)
			if err != nil {
				return nil, nil, err
			}
			value, err := g.processNode(is.Type)
			if err != nil {
				return nil, nil, err
			}

			record := g.Tracker.NewObjectLiteralExpression(g.Tracker.NewNodeList([]*ast.Node{
				g.Tracker.NewPropertyAssignment(nil, g.Tracker.NewIdentifier("key"), nil, nil, key),
				g.Tracker.NewPropertyAssignment(nil, g.Tracker.NewIdentifier("value"), nil, nil, value),
			}), false)
			records = append(records, record)
		}
	}
	return properties, records, nil
}

// ProcessInterfaceDeclaration generates schema statements for an interface declaration.
// When preferClass is true and there are no index signatures, generates a Schema.Class declaration.
// Otherwise generates a Schema.Struct variable declaration.
// Returns the generated statement node.
func (g *SchemaGen) ProcessInterfaceDeclaration(node *ast.Node, preferClass bool) (*ast.Node, error) {
	iface := node.AsInterfaceDeclaration()
	if iface.TypeParameters != nil && len(iface.TypeParameters.Nodes) > 0 {
		return nil, ErrTypeParametersNotSupported
	}

	name := scanner.GetTextOfNode(iface.Name())

	var memberNodes []*ast.Node
	if iface.Members != nil {
		memberNodes = iface.Members.Nodes
	}

	properties, records, err := g.processMembers(memberNodes)
	if err != nil {
		return nil, err
	}

	if preferClass && len(records) == 0 {
		return g.createExportSchemaClassDeclaration(name, properties), nil
	}

	schemaStruct := g.createApiCall("Struct",
		append([]*ast.Node{
			g.Tracker.NewObjectLiteralExpression(g.Tracker.NewNodeList(properties), true),
		}, records...),
	)
	return g.createExportVariableDeclaration(name, schemaStruct), nil
}

// ProcessTypeAliasDeclaration generates schema statements for a type alias declaration.
// When preferClass is true and the type is an object literal without index signatures,
// generates a Schema.Class declaration. Otherwise generates a variable declaration.
// Returns the generated statement node.
func (g *SchemaGen) ProcessTypeAliasDeclaration(node *ast.Node, preferClass bool) (*ast.Node, error) {
	ta := node.AsTypeAliasDeclaration()
	if ta.TypeParameters != nil && len(ta.TypeParameters.Nodes) > 0 {
		return nil, ErrTypeParametersNotSupported
	}

	name := scanner.GetTextOfNode(ta.Name())

	// For preferClass with object literal types, generate class declaration
	if preferClass && ta.Type != nil && ta.Type.Kind == ast.KindTypeLiteral {
		tl := ta.Type.AsTypeLiteralNode()
		var memberNodes []*ast.Node
		if tl.Members != nil {
			memberNodes = tl.Members.Nodes
		}
		properties, records, err := g.processMembers(memberNodes)
		if err != nil {
			return nil, err
		}
		if len(records) == 0 {
			return g.createExportSchemaClassDeclaration(name, properties), nil
		}
	}

	effectSchema, err := g.processNode(ta.Type)
	if err != nil {
		return nil, err
	}

	return g.createExportVariableDeclaration(name, effectSchema), nil
}

// createExportVariableDeclaration creates: export const <name> = <initializer>
func (g *SchemaGen) createExportVariableDeclaration(name string, initializer *ast.Node) *ast.Node {
	varDecl := g.Tracker.NewVariableDeclaration(
		g.Tracker.NewIdentifier(name),
		nil, // exclamationToken
		nil, // type
		initializer,
	)
	varDeclList := g.Tracker.NewVariableDeclarationList(
		g.Tracker.NewNodeList([]*ast.Node{varDecl}),
		ast.NodeFlagsConst,
	)
	modifiers := g.Tracker.NewModifierList([]*ast.Node{
		g.Tracker.NewModifier(ast.KindExportKeyword),
	})
	return g.Tracker.NewVariableStatement(modifiers, varDeclList)
}

// createExportSchemaClassDeclaration creates:
//
//	export class <Name> extends Schema.Class<Name>("Name")({...}) { }
func (g *SchemaGen) createExportSchemaClassDeclaration(name string, properties []*ast.Node) *ast.Node {
	// Schema.Class
	classAccess := g.createApiPropertyAccess("Class")

	// Schema.Class<Name>("Name")
	innerCall := g.Tracker.NewCallExpression(
		classAccess,
		nil, // questionDotToken
		g.Tracker.NewNodeList([]*ast.Node{
			g.Tracker.NewTypeReferenceNode(g.Tracker.NewIdentifier(name), nil),
		}),
		g.Tracker.NewNodeList([]*ast.Node{
			g.Tracker.NewStringLiteral(name, 0),
		}),
		ast.NodeFlagsNone,
	)

	// Schema.Class<Name>("Name")({...})
	outerCall := g.Tracker.NewCallExpression(
		innerCall,
		nil, nil,
		g.Tracker.NewNodeList([]*ast.Node{
			g.Tracker.NewObjectLiteralExpression(g.Tracker.NewNodeList(properties), true),
		}),
		ast.NodeFlagsNone,
	)

	// extends ...
	exprWithTypeArgs := g.Tracker.NewExpressionWithTypeArguments(
		outerCall,
		g.Tracker.NewNodeList([]*ast.Node{}),
	)
	heritageClause := g.Tracker.NewHeritageClause(
		ast.KindExtendsKeyword,
		g.Tracker.NewNodeList([]*ast.Node{exprWithTypeArgs}),
	)

	modifiers := g.Tracker.NewModifierList([]*ast.Node{
		g.Tracker.NewModifier(ast.KindExportKeyword),
	})

	return g.Tracker.NewClassDeclaration(
		modifiers,
		g.Tracker.NewIdentifier(name),
		nil, // typeParameters
		g.Tracker.NewNodeList([]*ast.Node{heritageClause}),
		g.Tracker.NewNodeList([]*ast.Node{}), // empty members
	)
}

// ProcessNode is the public entry point for processing a single type node.
// On error, returns a comment node describing the error.
func (g *SchemaGen) ProcessNode(node *ast.Node) *ast.Node {
	result, err := g.processNode(node)
	if err != nil {
		return g.Tracker.NewIdentifier(fmt.Sprintf("undefined /* %s */", err.Error()))
	}
	return result
}

// Process converts an interface or type alias declaration into a schema statement.
// On error, returns a comment node describing the error.
func (g *SchemaGen) Process(node *ast.Node, preferClass bool) *ast.Node {
	var result *ast.Node
	var err error

	switch node.Kind {
	case ast.KindInterfaceDeclaration:
		result, err = g.ProcessInterfaceDeclaration(node, preferClass)
	case ast.KindTypeAliasDeclaration:
		result, err = g.ProcessTypeAliasDeclaration(node, preferClass)
	default:
		return nil
	}

	if err != nil {
		return g.Tracker.NewIdentifier(fmt.Sprintf("undefined /* %s */", err.Error()))
	}

	ast.SetParentInChildren(result)
	return result
}

// HasIndexSignatures checks whether an interface or type alias has index signatures.
// Used by typeToEffectSchemaClass to determine applicability.
func HasIndexSignatures(node *ast.Node) bool {
	var members []*ast.Node
	switch node.Kind {
	case ast.KindInterfaceDeclaration:
		iface := node.AsInterfaceDeclaration()
		if iface.Members != nil {
			members = iface.Members.Nodes
		}
	case ast.KindTypeAliasDeclaration:
		ta := node.AsTypeAliasDeclaration()
		if ta.Type != nil && ta.Type.Kind == ast.KindTypeLiteral {
			tl := ta.Type.AsTypeLiteralNode()
			if tl.Members != nil {
				members = tl.Members.Nodes
			}
		}
	}
	for _, m := range members {
		if m.Kind == ast.KindIndexSignature {
			return true
		}
	}
	return false
}
