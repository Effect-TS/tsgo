// Package schemagen provides structural (type-checker-based) schema generation
// that converts resolved TypeScript types into Effect Schema expressions.
// Unlike the AST-based SchemaGen which works on syntax nodes, this generator works
// on actual types from the checker, enabling handling of recursive types and
// complex type hierarchies through a hoisting mechanism.
package schemagen

import (
	"errors"
	"fmt"
	"maps"
	"regexp"

	"github.com/effect-ts/tsgo/internal/effectutil"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

// StructuralSchemaGen holds the context for converting resolved types to Schema expressions.
type StructuralSchemaGen struct {
	Tracker          *change.Tracker
	SourceFile       *ast.SourceFile
	Checker          *checker.Checker
	SchemaIdentifier string
	Version          typeparser.EffectMajorVersion

	// hoistedSchemas maps types to expression generators for reuse
	hoistedSchemas map[checker.TypeId]hoistedEntry
	// typeToStatementIndex tracks which types have been hoisted to which statement index
	typeToStatementIndex map[checker.TypeId]int
	// nameToType maps requested names to their types
	nameToType map[string]*checker.Type
	// schemaStatements accumulates generated top-level statements
	schemaStatements []*ast.Node
	// usedGlobalIdentifiers tracks used identifier names and their count
	usedGlobalIdentifiers map[string]int
}

type hoistedEntry struct {
	t         *checker.Type
	createRef func() *ast.Node
}

// processingContext tracks state during recursive type traversal.
type processingContext struct {
	depth     int
	maxDepth  int
	hoistName string
	version   typeparser.EffectMajorVersion
}

// NewStructuralSchemaGen creates a StructuralSchemaGen for type-checker-based schema generation.
func NewStructuralSchemaGen(tracker *change.Tracker, sf *ast.SourceFile, c *checker.Checker, version typeparser.EffectMajorVersion) *StructuralSchemaGen {
	return &StructuralSchemaGen{
		Tracker:               tracker,
		SourceFile:            sf,
		Checker:               c,
		SchemaIdentifier:      effectutil.FindModuleIdentifier(sf, "Schema"),
		Version:               version,
		hoistedSchemas:        make(map[checker.TypeId]hoistedEntry),
		typeToStatementIndex:  make(map[checker.TypeId]int),
		nameToType:            make(map[string]*checker.Type),
		schemaStatements:      nil,
		usedGlobalIdentifiers: make(map[string]int),
	}
}

// createApiPropertyAccess creates Schema.<name>.
func (g *StructuralSchemaGen) createApiPropertyAccess(name string) *ast.Node {
	return g.Tracker.NewPropertyAccessExpression(
		g.Tracker.NewIdentifier(g.SchemaIdentifier),
		nil,
		g.Tracker.NewIdentifier(name),
		ast.NodeFlagsNone,
	)
}

// createApiCall creates Schema.<name>(...args).
func (g *StructuralSchemaGen) createApiCall(name string, args []*ast.Node) *ast.Node {
	return g.Tracker.NewCallExpression(
		g.Tracker.NewPropertyAccessExpression(
			g.Tracker.NewIdentifier(g.SchemaIdentifier),
			nil,
			g.Tracker.NewIdentifier(name),
			ast.NodeFlagsNone,
		),
		nil, nil,
		g.Tracker.NewNodeList(args),
		ast.NodeFlagsNone,
	)
}

// pushHoistedStatement records a generated statement and its reference generator.
func (g *StructuralSchemaGen) pushHoistedStatement(name string, t *checker.Type, statement *ast.Node, createRef func() *ast.Node) {
	g.usedGlobalIdentifiers[name]++
	g.schemaStatements = append(g.schemaStatements, statement)
	g.typeToStatementIndex[t.Id()] = len(g.schemaStatements) - 1
	g.hoistedSchemas[t.Id()] = hoistedEntry{t: t, createRef: createRef}
}

// pushHoistedVariableStatement creates a const variable statement and hoists it.
func (g *StructuralSchemaGen) pushHoistedVariableStatement(name string, t *checker.Type, result *ast.Node) {
	varDecl := g.Tracker.NewVariableDeclaration(
		g.Tracker.NewIdentifier(name),
		nil, nil,
		result,
	)
	varDeclList := g.Tracker.NewVariableDeclarationList(
		ast.NodeFlagsConst,
		g.Tracker.NewNodeList([]*ast.Node{varDecl}),
	)
	stmt := g.Tracker.NewVariableStatement(nil, varDeclList)
	g.pushHoistedStatement(name, t, stmt, func() *ast.Node {
		return g.Tracker.NewIdentifier(name)
	})
}

// processType converts a TypeScript type to a Schema expression, with hoisting support.
func (g *StructuralSchemaGen) processType(t *checker.Type, ctx processingContext) (*ast.Node, error) {
	if ctx.depth >= ctx.maxDepth {
		return nil, errors.New("maximum depth exceeded")
	}

	// Try to resolve a hoist name from the type
	hoistName := g.resolveHoistName(t)

	nestedCtx := processingContext{
		depth:     ctx.depth + 1,
		maxDepth:  ctx.maxDepth,
		hoistName: hoistName,
		version:   ctx.version,
	}

	// Check if we can reuse a hoisted schema
	for _, entry := range g.hoistedSchemas {
		if entry.t == t || g.typesEqual(t, entry.t) {
			return entry.createRef(), nil
		}
	}

	// Process the type
	expr, skipHoisting, err := g.processTypeImpl(t, nestedCtx)
	if err != nil {
		return nil, err
	}

	// Hoist if we have a name and hoisting isn't skipped
	if !skipHoisting && hoistName != "" {
		g.pushHoistedVariableStatement(hoistName, t, expr)
		if entry, ok := g.hoistedSchemas[t.Id()]; ok {
			return entry.createRef(), nil
		}
	}

	return expr, nil
}

// resolveHoistName tries to find a name for the type for hoisting purposes.
func (g *StructuralSchemaGen) resolveHoistName(t *checker.Type) string {
	// Check nameToType map first
	for name, existingType := range g.nameToType {
		if existingType == t {
			return name
		}
	}

	// Try to get name from the type's symbol
	if t.Symbol() != nil && len(t.Symbol().Declarations) == 1 {
		decl := t.Symbol().Declarations[0]
		var hoistName string
		if decl.Kind == ast.KindInterfaceDeclaration {
			hoistName = decl.AsInterfaceDeclaration().Name().AsIdentifier().Text
		} else if decl.Parent != nil && decl.Parent.Kind == ast.KindTypeAliasDeclaration {
			hoistName = decl.Parent.AsTypeAliasDeclaration().Name().AsIdentifier().Text
		}
		if hoistName != "" {
			existingType := g.nameToType[hoistName]
			if existingType != nil && g.typesEqual(t, existingType) {
				return hoistName
			}
			// Avoid name conflicts
			usedCount := g.usedGlobalIdentifiers[hoistName]
			g.usedGlobalIdentifiers[hoistName] = usedCount + 1
			if usedCount > 0 {
				return fmt.Sprintf("%s_%d", hoistName, usedCount)
			}
			return hoistName
		}
	}

	return ""
}

// typesEqual checks if two types are bidirectionally assignable.
func (g *StructuralSchemaGen) typesEqual(a, b *checker.Type) bool {
	return checker.Checker_isTypeAssignableTo(g.Checker, a, b) &&
		checker.Checker_isTypeAssignableTo(g.Checker, b, a)
}

// processTypeImpl dispatches type processing based on type flags.
func (g *StructuralSchemaGen) processTypeImpl(t *checker.Type, ctx processingContext) (expr *ast.Node, skipHoisting bool, err error) {
	flags := t.Flags()

	// Primitive types
	if flags&checker.TypeFlagsString != 0 {
		return g.createApiPropertyAccess("String"), true, nil
	}
	if flags&checker.TypeFlagsNumber != 0 {
		return g.createApiPropertyAccess("Number"), true, nil
	}
	if flags&checker.TypeFlagsBoolean != 0 {
		return g.createApiPropertyAccess("Boolean"), true, nil
	}
	if flags&checker.TypeFlagsBigInt != 0 {
		return g.createApiPropertyAccess("BigInt"), true, nil
	}
	if flags&checker.TypeFlagsVoid != 0 {
		return g.createApiPropertyAccess("Void"), true, nil
	}
	if flags&checker.TypeFlagsUndefined != 0 {
		return g.createApiPropertyAccess("Undefined"), true, nil
	}
	if flags&checker.TypeFlagsNull != 0 {
		return g.createApiPropertyAccess("Null"), true, nil
	}
	if flags&checker.TypeFlagsNever != 0 {
		return g.createApiPropertyAccess("Never"), true, nil
	}
	if flags&checker.TypeFlagsAny != 0 {
		return g.createApiPropertyAccess("Any"), true, nil
	}
	if flags&checker.TypeFlagsUnknown != 0 {
		return g.createApiPropertyAccess("Unknown"), true, nil
	}

	// Literal types
	if flags&checker.TypeFlagsStringLiteral != 0 {
		val, ok := t.AsLiteralType().Value().(string)
		if ok {
			return g.createApiCall("Literal", []*ast.Node{g.Tracker.NewStringLiteral(val, 0)}), true, nil
		}
	}
	if flags&checker.TypeFlagsNumberLiteral != 0 {
		return g.createApiCall("Literal", []*ast.Node{
			g.Tracker.NewNumericLiteral(t.AsLiteralType().String(), 0),
		}), true, nil
	}
	if flags&checker.TypeFlagsBooleanLiteral != 0 {
		val, ok := t.AsLiteralType().Value().(bool)
		if ok && val {
			return g.createApiCall("Literal", []*ast.Node{g.Tracker.NewKeywordExpression(ast.KindTrueKeyword)}), true, nil
		}
		return g.createApiCall("Literal", []*ast.Node{g.Tracker.NewKeywordExpression(ast.KindFalseKeyword)}), true, nil
	}

	// Union types
	if flags&checker.TypeFlagsUnion != 0 {
		types := t.Types()
		result, skip, err := g.processUnionType(types, ctx)
		return result, skip, err
	}

	// Intersection types
	if flags&checker.TypeFlagsIntersection != 0 {
		types := t.Types()
		result, skip, err := g.processIntersectionType(types, ctx)
		return result, skip, err
	}

	// Array types (check before generic object)
	if checker.Checker_isArrayType(g.Checker, t) {
		result, skip, err := g.processArrayType(t, ctx)
		return result, skip, err
	}

	// Tuple types
	if checker.IsTupleType(t) {
		result, skip, err := g.processTupleType(t, ctx)
		return result, skip, err
	}

	// Object types
	if flags&checker.TypeFlagsObject != 0 {
		sym := t.Symbol()
		if sym != nil {
			typeName := g.Checker.SymbolToString(sym)
			if typeName == "Date" {
				return g.createApiPropertyAccess("Date"), false, nil
			}
			if typeName == "ReadonlyArray" || typeName == "Array" {
				result, skip, err := g.processArrayType(t, ctx)
				return result, skip, err
			}
		}
		result, skip, err := g.processObjectType(t, ctx)
		return result, skip, err
	}

	return nil, false, fmt.Errorf("type with flags %d is not supported", flags)
}

// processUnionType converts union types to Schema.Union or optimized Schema.Literal.
func (g *StructuralSchemaGen) processUnionType(types []*checker.Type, ctx processingContext) (*ast.Node, bool, error) {
	// Check if all members are literals (V3 optimization)
	allLiterals := true
	for _, t := range types {
		f := t.Flags()
		if f&checker.TypeFlagsStringLiteral == 0 && f&checker.TypeFlagsNumberLiteral == 0 && f&checker.TypeFlagsBooleanLiteral == 0 {
			allLiterals = false
			break
		}
	}

	if allLiterals && ctx.version != typeparser.EffectMajorV4 {
		literals := make([]*ast.Node, 0, len(types))
		for _, t := range types {
			expr, err := g.processType(t, ctx)
			if err != nil {
				return nil, false, err
			}
			// Extract literal value from Schema.Literal(value) call
			if expr.Kind == ast.KindCallExpression {
				ce := expr.AsCallExpression()
				if ce.Arguments != nil && len(ce.Arguments.Nodes) > 0 {
					literals = append(literals, ce.Arguments.Nodes[0])
					continue
				}
			}
			literals = append(literals, expr)
		}
		return g.createApiCall("Literal", literals), false, nil
	}

	// Process each union member
	members := make([]*ast.Node, 0, len(types))
	for _, t := range types {
		expr, err := g.processType(t, ctx)
		if err != nil {
			return nil, false, err
		}
		members = append(members, expr)
	}

	if len(members) == 1 {
		return members[0], false, nil
	}

	var args []*ast.Node
	if ctx.version == typeparser.EffectMajorV4 {
		args = []*ast.Node{g.Tracker.NewArrayLiteralExpression(g.Tracker.NewNodeList(members), false)}
	} else {
		args = members
	}
	return g.createApiCall("Union", args), false, nil
}

// processIntersectionType converts intersection types to Schema.extend chains.
func (g *StructuralSchemaGen) processIntersectionType(types []*checker.Type, ctx processingContext) (*ast.Node, bool, error) {
	members := make([]*ast.Node, 0, len(types))
	for _, t := range types {
		expr, err := g.processType(t, ctx)
		if err != nil {
			return nil, false, err
		}
		members = append(members, expr)
	}

	if len(members) == 0 {
		return nil, false, errors.New("empty intersection type")
	}
	if len(members) == 1 {
		return members[0], false, nil
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
	), false, nil
}

// processArrayType converts array types to Schema.Array (wrapping in Schema.mutable for non-readonly).
func (g *StructuralSchemaGen) processArrayType(t *checker.Type, ctx processingContext) (*ast.Node, bool, error) {
	typeArgs := checker.Checker_getTypeArguments(g.Checker, t)
	if len(typeArgs) == 0 {
		return nil, false, errors.New("array type has no type arguments")
	}

	elemSchema, err := g.processType(typeArgs[0], ctx)
	if err != nil {
		return nil, false, err
	}
	expr := g.createApiCall("Array", []*ast.Node{elemSchema})
	if checker.Checker_isReadonlyArrayType(g.Checker, t) {
		return expr, false, nil
	}
	return g.createApiCall("mutable", []*ast.Node{expr}), false, nil
}

// processTupleType converts tuple types to Schema.Tuple.
func (g *StructuralSchemaGen) processTupleType(t *checker.Type, ctx processingContext) (*ast.Node, bool, error) {
	typeArgs := checker.Checker_getTypeArguments(g.Checker, t)
	elems := make([]*ast.Node, 0, len(typeArgs))
	for _, ta := range typeArgs {
		expr, err := g.processType(ta, ctx)
		if err != nil {
			return nil, false, err
		}
		elems = append(elems, expr)
	}

	var args []*ast.Node
	if ctx.version == typeparser.EffectMajorV4 {
		args = []*ast.Node{g.Tracker.NewArrayLiteralExpression(g.Tracker.NewNodeList(elems), false)}
	} else {
		args = elems
	}
	return g.createApiCall("Tuple", args), false, nil
}

// validIdentifierRegex matches valid JS identifiers.
var validIdentifierRegex = regexp.MustCompile(`^[a-zA-Z_$][a-zA-Z0-9_$]*$`)

// processObjectType converts object types to Schema.Struct or Schema.Class.
func (g *StructuralSchemaGen) processObjectType(t *checker.Type, ctx processingContext) (*ast.Node, bool, error) {
	properties := g.Checker.GetPropertiesOfType(t)
	propertyAssignments := make([]*ast.Node, 0, len(properties))

	for _, prop := range properties {
		propName := g.Checker.SymbolToString(prop)
		propType := g.Checker.GetTypeOfSymbol(prop)

		isOptional := prop.Flags&ast.SymbolFlagsOptional != 0

		var schemaExpr *ast.Node
		if isOptional {
			innerExpr, err := g.processType(propType, ctx)
			if err != nil {
				return nil, false, err
			}
			schemaExpr = g.createApiCall("optional", []*ast.Node{innerExpr})
		} else {
			var err error
			schemaExpr, err = g.processType(propType, ctx)
			if err != nil {
				return nil, false, err
			}
		}

		// Create property name node
		var propNameNode *ast.Node
		if validIdentifierRegex.MatchString(propName) {
			propNameNode = g.Tracker.NewIdentifier(propName)
		} else {
			propNameNode = g.Tracker.NewStringLiteral(propName, 0)
		}

		assignment := g.Tracker.NewPropertyAssignment(nil, propNameNode, nil, nil, schemaExpr)
		propertyAssignments = append(propertyAssignments, assignment)
	}

	// Handle index signatures
	indexInfos := checker.Checker_getIndexInfosOfType(g.Checker, t)
	args := []*ast.Node{
		g.Tracker.NewObjectLiteralExpression(g.Tracker.NewNodeList(propertyAssignments), len(propertyAssignments) > 0),
	}

	type recordEntry struct {
		key   *ast.Node
		value *ast.Node
	}
	var records []recordEntry
	for _, info := range indexInfos {
		keyType := checker.IndexInfo_keyType(info)
		valueType := checker.IndexInfo_valueType(info)

		keySchema, err := g.processType(keyType, ctx)
		if err != nil {
			return nil, false, err
		}
		valueSchema, err := g.processType(valueType, ctx)
		if err != nil {
			return nil, false, err
		}
		records = append(records, recordEntry{key: keySchema, value: valueSchema})
	}

	// V4: use StructWithRest for structs with index signatures
	if ctx.version == typeparser.EffectMajorV4 && len(records) > 0 {
		recordExprs := make([]*ast.Node, 0, len(records))
		for _, r := range records {
			recordExprs = append(recordExprs, g.createApiCall("Record", []*ast.Node{r.key, r.value}))
		}
		return g.createApiCall("StructWithRest", []*ast.Node{
			g.createApiCall("Struct", args),
			g.Tracker.NewArrayLiteralExpression(g.Tracker.NewNodeList(recordExprs), false),
		}), len(propertyAssignments) == 0, nil
	}

	// When no records and we have a hoistName, generate Schema.Class
	if len(records) == 0 && ctx.hoistName != "" {
		classAccess := g.createApiPropertyAccess("Class")
		innerCall := g.Tracker.NewCallExpression(
			classAccess,
			nil,
			g.Tracker.NewNodeList([]*ast.Node{
				g.Tracker.NewTypeReferenceNode(g.Tracker.NewIdentifier(ctx.hoistName), nil),
			}),
			g.Tracker.NewNodeList([]*ast.Node{
				g.Tracker.NewStringLiteral(ctx.hoistName, 0),
			}),
			ast.NodeFlagsNone,
		)
		outerCall := g.Tracker.NewCallExpression(
			innerCall,
			nil, nil,
			g.Tracker.NewNodeList(args),
			ast.NodeFlagsNone,
		)
		exprWithTypeArgs := g.Tracker.NewExpressionWithTypeArguments(
			outerCall,
			g.Tracker.NewNodeList([]*ast.Node{}),
		)
		heritageClause := g.Tracker.NewHeritageClause(
			ast.KindExtendsKeyword,
			g.Tracker.NewNodeList([]*ast.Node{exprWithTypeArgs}),
		)
		classDecl := g.Tracker.NewClassDeclaration(
			nil, // modifiers (export added later)
			g.Tracker.NewIdentifier(ctx.hoistName),
			nil,
			g.Tracker.NewNodeList([]*ast.Node{heritageClause}),
			g.Tracker.NewNodeList([]*ast.Node{}),
		)

		g.pushHoistedStatement(ctx.hoistName, t, classDecl, func() *ast.Node {
			return g.Tracker.NewIdentifier(ctx.hoistName)
		})
		if entry, ok := g.hoistedSchemas[t.Id()]; ok {
			return entry.createRef(), true, nil
		}
		return g.Tracker.NewIdentifier(ctx.hoistName), true, nil
	}

	// V3: records go as additional arguments to Schema.Struct
	for _, r := range records {
		args = append(args, g.Tracker.NewObjectLiteralExpression(g.Tracker.NewNodeList([]*ast.Node{
			g.Tracker.NewPropertyAssignment(nil, g.Tracker.NewIdentifier("key"), nil, nil, r.key),
			g.Tracker.NewPropertyAssignment(nil, g.Tracker.NewIdentifier("value"), nil, nil, r.value),
		}), false))
	}

	return g.createApiCall("Struct", args), len(propertyAssignments) == 0, nil
}

// ScanExistingSchemas looks for existing schema variables in scope and records them
// in the hoisted map, so they can be reused instead of regenerated.
func (g *StructuralSchemaGen) ScanExistingSchemas(scope *ast.Node) {
	symbols := g.Checker.GetSymbolsInScope(scope, ast.SymbolFlagsValue)
	for _, sym := range symbols {
		name := g.Checker.SymbolToString(sym)
		g.usedGlobalIdentifiers[name] = 1
		t := g.Checker.GetTypeOfSymbolAtLocation(sym, g.SourceFile.AsNode())
		if t == nil {
			continue
		}
		schemaTypes := typeparser.EffectSchemaTypes(g.Checker, t, scope)
		if schemaTypes == nil {
			continue
		}
		aType := schemaTypes.A
		capturedName := name
		g.hoistedSchemas[aType.Id()] = hoistedEntry{
			t: aType,
			createRef: func() *ast.Node {
				return g.Tracker.NewIdentifier(capturedName)
			},
		}
	}
}

// Process generates schema statements for one or more named types.
// It populates internal state and returns accumulated statements.
func (g *StructuralSchemaGen) Process(typeMap map[string]*checker.Type, scope *ast.Node, isExported bool) []*ast.Node {
	maps.Copy(g.nameToType, typeMap)

	// Scan existing schemas in scope for reuse
	g.ScanExistingSchemas(scope)

	// Process each type
	type processResult struct {
		requestedName string
		t             *checker.Type
		result        *ast.Node
	}

	var results []processResult
	for name, t := range typeMap {
		ctx := processingContext{
			depth:    0,
			maxDepth: 200,
			version:  g.Version,
		}
		expr, err := g.processType(t, ctx)
		if err != nil {
			// On error, emit an error comment
			expr = g.Tracker.NewIdentifier(fmt.Sprintf("undefined /* %s */", err.Error()))
		}
		results = append(results, processResult{
			requestedName: name,
			t:             t,
			result:        expr,
		})
	}

	// Add variable statements for types that were not hoisted
	for _, r := range results {
		if _, ok := g.typeToStatementIndex[r.t.Id()]; ok {
			continue
		}
		varDecl := g.Tracker.NewVariableDeclaration(
			g.Tracker.NewIdentifier(r.requestedName),
			nil, nil,
			r.result,
		)
		varDeclList := g.Tracker.NewVariableDeclarationList(
			ast.NodeFlagsConst,
			g.Tracker.NewNodeList([]*ast.Node{varDecl}),
		)
		modifiers := g.Tracker.NewModifierList([]*ast.Node{
			g.Tracker.NewModifier(ast.KindExportKeyword),
		})
		stmt := g.Tracker.NewVariableStatement(modifiers, varDeclList)
		g.schemaStatements = append(g.schemaStatements, stmt)
		g.typeToStatementIndex[r.t.Id()] = len(g.schemaStatements) - 1
	}

	// Add export modifiers to requested type statements
	if isExported {
		exportIndices := make(map[int]bool)
		for _, t := range g.nameToType {
			if idx, ok := g.typeToStatementIndex[t.Id()]; ok {
				exportIndices[idx] = true
			}
		}
		for i := range g.schemaStatements {
			if !exportIndices[i] {
				continue
			}
			stmt := g.schemaStatements[i]
			if stmt.Kind == ast.KindVariableStatement {
				// Already has export modifier from non-hoisted path; skip if so
				vs := stmt.AsVariableStatement()
				if vs.Modifiers() != nil && len(vs.Modifiers().Nodes) > 0 {
					continue
				}
				modifiers := g.Tracker.NewModifierList([]*ast.Node{
					g.Tracker.NewModifier(ast.KindExportKeyword),
				})
				g.schemaStatements[i] = g.Tracker.NewVariableStatement(modifiers, vs.DeclarationList)
			} else if stmt.Kind == ast.KindClassDeclaration {
				cd := stmt.AsClassDeclaration()
				if cd.Modifiers() != nil && len(cd.Modifiers().Nodes) > 0 {
					continue
				}
				modifiers := g.Tracker.NewModifierList([]*ast.Node{
					g.Tracker.NewModifier(ast.KindExportKeyword),
				})
				g.schemaStatements[i] = g.Tracker.NewClassDeclaration(
					modifiers,
					cd.Name(),
					cd.TypeParameters,
					cd.HeritageClauses,
					cd.Members,
				)
			}
		}
	}

	// Set parent pointers
	for _, stmt := range g.schemaStatements {
		ast.SetParentInChildren(stmt)
	}

	return g.schemaStatements
}

// Statements returns the accumulated schema statements.
func (g *StructuralSchemaGen) Statements() []*ast.Node {
	return g.schemaStatements
}
