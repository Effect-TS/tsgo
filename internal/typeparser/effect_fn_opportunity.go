// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/effect-ts/effect-typescript-go/internal/checkerutils"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// IsInsideEffectFn checks if a function node is already the first argument
// of an Effect.fn, Effect.fnGen, or Effect.fnUntraced call.
func IsInsideEffectFn(c *checker.Checker, fnNode *ast.Node) bool {
	if c == nil || fnNode == nil {
		return false
	}

	parent := fnNode.Parent
	if parent == nil || parent.Kind != ast.KindCallExpression {
		return false
	}

	parentCall := parent.AsCallExpression()
	if parentCall == nil || parentCall.Arguments == nil || len(parentCall.Arguments.Nodes) == 0 {
		return false
	}

	// The function must be the first argument
	if parentCall.Arguments.Nodes[0] != fnNode {
		return false
	}

	if EffectFnCall(c, parent) != nil {
		return true
	}
	if EffectFnGenCall(c, parent) != nil {
		return true
	}
	if EffectFnUntracedGenCall(c, parent) != nil {
		return true
	}

	return false
}

// ParseEffectFnOpportunity detects whether a function node can be converted to Effect.fn.
// Returns nil if the node is not an eligible candidate.
func ParseEffectFnOpportunity(c *checker.Checker, node *ast.Node) *EffectFnOpportunityResult {
	if c == nil || node == nil {
		return nil
	}

	// Step 1: Filter by node kind
	switch node.Kind {
	case ast.KindFunctionExpression, ast.KindArrowFunction, ast.KindFunctionDeclaration:
		// OK
	default:
		return nil
	}

	// Step 2: Reject generators
	if node.Kind == ast.KindFunctionExpression {
		fn := node.AsFunctionExpression()
		if fn != nil && fn.AsteriskToken != nil {
			return nil
		}
	}
	if node.Kind == ast.KindFunctionDeclaration {
		fn := node.AsFunctionDeclaration()
		if fn != nil && fn.AsteriskToken != nil {
			return nil
		}
	}

	// Step 3: Reject named function expressions (typically used for recursion)
	if node.Kind == ast.KindFunctionExpression {
		fn := node.AsFunctionExpression()
		if fn != nil && fn.Name() != nil {
			return nil
		}
	}

	// Step 4: Reject functions with return type annotations
	if hasReturnTypeAnnotation(node) {
		return nil
	}

	// Step 5: Reject if already inside Effect.fn
	if IsInsideEffectFn(c, node) {
		return nil
	}

	// Step 6: Get function type, require exactly 1 call signature
	functionType := checkerutils.GetTypeAtLocation(c,node)
	if functionType == nil {
		return nil
	}
	callSignatures := c.GetSignaturesOfType(functionType, checker.SignatureKindCall)
	if len(callSignatures) != 1 {
		return nil
	}

	// Step 7: Get return type, unroll union members, require all are strict Effect types
	returnType := c.GetReturnTypeOfSignature(callSignatures[0])
	if returnType == nil {
		return nil
	}
	unionMembers := UnrollUnionMembers(returnType)
	if len(unionMembers) == 0 {
		return nil
	}
	for _, member := range unionMembers {
		if StrictEffectType(c, member, node) == nil {
			return nil
		}
	}

	// Step 8: Extract name identifier from context
	nameIdentifier := ast.GetNameOfDeclaration(node)
	// GetNameOfDeclaration doesn't cover property declaration parents for arrow/function expressions
	if nameIdentifier == nil && node.Parent != nil && node.Parent.Kind == ast.KindPropertyDeclaration {
		pd := node.Parent.AsPropertyDeclaration()
		if pd != nil {
			nameIdentifier = pd.Name()
		}
	}
	// Also check PropertyAssignment parents (e.g., object literal members like { query: (...) => Effect... })
	if nameIdentifier == nil && node.Parent != nil && node.Parent.Kind == ast.KindPropertyAssignment {
		pa := node.Parent.AsPropertyAssignment()
		if pa != nil && pa.Initializer == node {
			nameIdentifier = pa.Name()
		}
	}
	if nameIdentifier != nil && nameIdentifier.Kind != ast.KindIdentifier && nameIdentifier.Kind != ast.KindStringLiteral {
		nameIdentifier = nil
	}
	if nameIdentifier == nil {
		return nil
	}
	traceName := scanner.GetTextOfNode(nameIdentifier)
	if traceName == "" {
		return nil
	}

	// Step 8b: Compute suggestedTraceName (local name) and inferredTraceName (context-aware name)
	suggestedTraceName := traceName
	inferredTraceName := getInferredTraceName(c, node, suggestedTraceName)

	// Detect whether the target function is a property value inside a Layer service definition
	isLayerMember := inferredTraceName != "" && inferredTraceName != suggestedTraceName

	// Step 9: Try gen opportunity first
	if result := tryParseGenOpportunity(c, node); result != nil {
		// Safety check: reject if function parameters are referenced in pipe arguments
		if len(result.pipeArguments) > 0 && areParametersReferencedIn(c, node, result.pipeArguments) {
			return nil
		}
		return &EffectFnOpportunityResult{
			TargetNode:              node,
			NameIdentifier:          nameIdentifier,
			GeneratorFunction:       result.generatorFunction,
			PipeArguments:           result.pipeArguments,
			ExplicitTraceExpression: result.explicitTraceExpression,
			SuggestedTraceName:      suggestedTraceName,
			InferredTraceName:       inferredTraceName,
			EffectModule:            result.effectModule,
			HasGenBody:              true,
			IsLayerMember:           isLayerMember,
		}
	}

	// Step 10: Try regular opportunity (block body with >5 statements, relaxed in Layer context)
	if result := tryParseRegularOpportunity(node, isLayerMember); result {
		return &EffectFnOpportunityResult{
			TargetNode:         node,
			NameIdentifier:     nameIdentifier,
			SuggestedTraceName: suggestedTraceName,
			InferredTraceName:  inferredTraceName,
			HasGenBody:         false,
			IsLayerMember:      isLayerMember,
		}
	}

	return nil
}

// hasReturnTypeAnnotation checks if a function node has an explicit return type annotation.
func hasReturnTypeAnnotation(node *ast.Node) bool {
	switch node.Kind {
	case ast.KindArrowFunction:
		fn := node.AsArrowFunction()
		if fn != nil && fn.Type != nil {
			return true
		}
	case ast.KindFunctionExpression:
		fn := node.AsFunctionExpression()
		if fn != nil && fn.Type != nil {
			return true
		}
	case ast.KindFunctionDeclaration:
		fn := node.AsFunctionDeclaration()
		if fn != nil && fn.Type != nil {
			return true
		}
	}
	return false
}

// genOpportunityResult holds the parsed gen opportunity data.
type genOpportunityResult struct {
	effectModule           *ast.Expression
	generatorFunction      *ast.FunctionExpression
	pipeArguments          []*ast.Node
	explicitTraceExpression *ast.Node
}

// tryParseGenOpportunity attempts to parse a function as a gen opportunity.
// The function body must contain a single return statement (or expression body for arrows)
// that is an Effect.gen call (possibly piped).
func tryParseGenOpportunity(c *checker.Checker, fnNode *ast.Node) *genOpportunityResult {
	bodyExpr := getBodyExpression(fnNode)
	if bodyExpr == nil {
		return nil
	}

	// Try to parse as a pipe call first to get subject and pipe args
	var subject *ast.Node
	var pipeArgs []*ast.Node

	if pipeResult := ParsePipeCall(c, bodyExpr); pipeResult != nil {
		subject = pipeResult.Subject
		pipeArgs = pipeResult.Args
	} else {
		subject = bodyExpr
	}

	// The subject must be an Effect.gen call
	genResult := effectGenFirstArgOnly(c, subject)
	if genResult == nil {
		return nil
	}

	// Check if the last pipe argument is Effect.withSpan
	var explicitTraceExpression *ast.Node
	if len(pipeArgs) > 0 {
		lastArg := pipeArgs[len(pipeArgs)-1]
		if withSpanExpr := tryExtractWithSpanExpression(c, lastArg); withSpanExpr != nil {
			explicitTraceExpression = withSpanExpr
		}
	}

	return &genOpportunityResult{
		effectModule:            genResult.EffectModule,
		generatorFunction:       genResult.GeneratorFunction,
		pipeArguments:           pipeArgs,
		explicitTraceExpression: explicitTraceExpression,
	}
}

// effectGenFirstArgOnly parses a node as Effect.gen(<generator>) where the generator
// is the FIRST argument. Unlike EffectGenCall which scans all arguments, this rejects
// cases like Effect.gen({self: this}, function*(){}) where the generator is not first.
func effectGenFirstArgOnly(c *checker.Checker, node *ast.Node) *EffectGenCallResult {
	if c == nil || node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}

	call := node.AsCallExpression()
	if call == nil || call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
		return nil
	}

	// The first argument must be a generator function expression
	firstArg := call.Arguments.Nodes[0]
	if firstArg == nil || firstArg.Kind != ast.KindFunctionExpression {
		return nil
	}
	genFn := firstArg.AsFunctionExpression()
	if genFn == nil || genFn.AsteriskToken == nil {
		return nil
	}

	expr := call.Expression
	if expr == nil || expr.Kind != ast.KindPropertyAccessExpression {
		return nil
	}

	propertyAccess := expr.AsPropertyAccessExpression()
	if propertyAccess == nil {
		return nil
	}

	if !IsNodeReferenceToEffectModuleApi(c, expr, "gen") {
		return nil
	}

	return &EffectGenCallResult{
		Call:              call,
		EffectModule:      propertyAccess.Expression,
		GeneratorFunction: genFn,
		Body:              genFn.Body,
	}
}

// getBodyExpression gets the single return expression from a function body.
// For arrow functions with expression bodies, returns the expression directly.
// For block bodies, requires exactly one return statement.
func getBodyExpression(fnNode *ast.Node) *ast.Node {
	switch fnNode.Kind {
	case ast.KindArrowFunction:
		fn := fnNode.AsArrowFunction()
		if fn == nil || fn.Body == nil {
			return nil
		}
		if fn.Body.Kind == ast.KindBlock {
			return findSingleReturnExpression(fn.Body)
		}
		return fn.Body

	case ast.KindFunctionExpression:
		fn := fnNode.AsFunctionExpression()
		if fn == nil || fn.Body == nil {
			return nil
		}
		return findSingleReturnExpression(fn.Body)

	case ast.KindFunctionDeclaration:
		fn := fnNode.AsFunctionDeclaration()
		if fn == nil || fn.Body == nil {
			return nil
		}
		return findSingleReturnExpression(fn.Body)
	}
	return nil
}

// findSingleReturnExpression finds the expression from a single return statement in a block.
func findSingleReturnExpression(body *ast.Node) *ast.Node {
	if body == nil || body.Kind != ast.KindBlock {
		return nil
	}
	block := body.AsBlock()
	if block == nil || block.Statements == nil || len(block.Statements.Nodes) != 1 {
		return nil
	}
	stmt := block.Statements.Nodes[0]
	if stmt == nil || stmt.Kind != ast.KindReturnStatement {
		return nil
	}
	return stmt.AsReturnStatement().Expression
}

// tryParseRegularOpportunity checks if a function has a block body with more than 5 statements.
// When hasStrictLayerInferredName is true, the >5 statement requirement and the block body
// requirement for arrow functions are relaxed (any Effect-returning function in Layer context qualifies).
func tryParseRegularOpportunity(fnNode *ast.Node, hasStrictLayerInferredName bool) bool {
	switch fnNode.Kind {
	case ast.KindArrowFunction:
		fn := fnNode.AsArrowFunction()
		if fn == nil || fn.Body == nil {
			return false
		}
		// Arrow with concise body (expression, no block): only allowed in Layer context
		if fn.Body.Kind != ast.KindBlock {
			return hasStrictLayerInferredName
		}
		if hasStrictLayerInferredName {
			return true
		}
		block := fn.Body.AsBlock()
		return block != nil && block.Statements != nil && len(block.Statements.Nodes) > 5

	case ast.KindFunctionExpression:
		fn := fnNode.AsFunctionExpression()
		if fn == nil || fn.Body == nil || fn.Body.Kind != ast.KindBlock {
			return false
		}
		if hasStrictLayerInferredName {
			return true
		}
		block := fn.Body.AsBlock()
		return block != nil && block.Statements != nil && len(block.Statements.Nodes) > 5

	case ast.KindFunctionDeclaration:
		fn := fnNode.AsFunctionDeclaration()
		if fn == nil || fn.Body == nil || fn.Body.Kind != ast.KindBlock {
			return false
		}
		if hasStrictLayerInferredName {
			return true
		}
		block := fn.Body.AsBlock()
		return block != nil && block.Statements != nil && len(block.Statements.Nodes) > 5
	}
	return false
}

// tryExtractWithSpanExpression checks if an expression is a call to Effect.withSpan
// and extracts the span name expression (the first argument).
func tryExtractWithSpanExpression(c *checker.Checker, expr *ast.Node) *ast.Node {
	if expr == nil || expr.Kind != ast.KindCallExpression {
		return nil
	}

	call := expr.AsCallExpression()
	if call == nil || call.Expression == nil {
		return nil
	}

	if !IsNodeReferenceToEffectModuleApi(c, call.Expression, "withSpan") {
		return nil
	}

	// withSpan has at least one argument (the span name)
	if call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
		return nil
	}

	return call.Arguments.Nodes[0]
}

// areParametersReferencedIn checks if any of the function's parameter symbols
// are referenced within the given nodes. Uses declaration position range checking.
func areParametersReferencedIn(c *checker.Checker, fnNode *ast.Node, nodes []*ast.Node) bool {
	if len(nodes) == 0 {
		return false
	}

	params := getFunctionParameters(fnNode)
	if len(params) == 0 {
		return false
	}

	// Get the position range of all parameters
	firstParam := params[0]
	lastParam := params[len(params)-1]
	paramsStart := firstParam.Pos()
	paramsEnd := lastParam.End()

	// Walk all nodes looking for symbols declared in the function parameters
	queue := make([]*ast.Node, len(nodes))
	copy(queue, nodes)

	for len(queue) > 0 {
		current := queue[0]
		queue = queue[1:]

		if current == nil {
			continue
		}

		// Check identifiers
		if current.Kind == ast.KindIdentifier {
			sym := c.GetSymbolAtLocation(current)
			if sym != nil && isSymbolDeclaredInRange(sym, paramsStart, paramsEnd) {
				return true
			}
		}

		// Check shorthand property assignments like { a, b }
		if current.Kind == ast.KindShorthandPropertyAssignment {
			spa := current.AsShorthandPropertyAssignment()
			if spa != nil {
				valueSym := c.GetShorthandAssignmentValueSymbol(current)
				if valueSym != nil && isSymbolDeclaredInRange(valueSym, paramsStart, paramsEnd) {
					return true
				}
			}
		}

		for child := range current.IterChildren() {
			queue = append(queue, child)
		}
	}

	return false
}

// isSymbolDeclaredInRange checks if any of the symbol's declarations fall within the given range.
func isSymbolDeclaredInRange(sym *ast.Symbol, start, end int) bool {
	for _, decl := range sym.Declarations {
		if decl != nil && decl.Pos() >= start && decl.End() <= end {
			return true
		}
	}
	return false
}

// getFunctionParameters returns the parameter nodes of a function.
func getFunctionParameters(fnNode *ast.Node) []*ast.Node {
	switch fnNode.Kind {
	case ast.KindArrowFunction:
		fn := fnNode.AsArrowFunction()
		if fn != nil && fn.Parameters != nil {
			return fn.Parameters.Nodes
		}
	case ast.KindFunctionExpression:
		fn := fnNode.AsFunctionExpression()
		if fn != nil && fn.Parameters != nil {
			return fn.Parameters.Nodes
		}
	case ast.KindFunctionDeclaration:
		fn := fnNode.AsFunctionDeclaration()
		if fn != nil && fn.Parameters != nil {
			return fn.Parameters.Nodes
		}
	}
	return nil
}

// tryGetLayerApiMethod checks if a node references Layer.effect, Layer.succeed, or Layer.sync.
// Returns the method name ("effect", "succeed", "sync") or empty string.
func tryGetLayerApiMethod(c *checker.Checker, node *ast.Node) string {
	if c == nil || node == nil {
		return ""
	}
	if IsNodeReferenceToEffectLayerModuleApi(c, node, "effect") {
		return "effect"
	}
	if IsNodeReferenceToEffectLayerModuleApi(c, node, "succeed") {
		return "succeed"
	}
	if IsNodeReferenceToEffectLayerModuleApi(c, node, "sync") {
		return "sync"
	}
	return ""
}

// layerServiceNameFromExpression resolves an expression to a service name string.
// When the expression is the `this` keyword, it walks up the AST to find the enclosing
// class declaration and returns the class name instead of literal "this".
func layerServiceNameFromExpression(expr *ast.Node) string {
	if expr == nil {
		return ""
	}
	if expr.Kind == ast.KindThisKeyword {
		classDecl := ast.FindAncestorKind(expr, ast.KindClassDeclaration)
		if classDecl != nil && classDecl.Name() != nil {
			return scanner.GetTextOfNode(classDecl.Name())
		}
	}
	return scanner.GetTextOfNode(expr)
}

// verifyLayerMethodAtCall checks if a call expression is a Layer method call (direct or curried)
// and returns the tag text (e.g., "MyService") or empty string.
// Direct form: Layer.method(tag, impl) where impl === implementationExpr
// Curried form: Layer.method(tag)(impl) where impl === implementationExpr
func verifyLayerMethodAtCall(c *checker.Checker, callExpr *ast.CallExpression, method string, implementationExpr *ast.Node) string {
	if callExpr == nil || callExpr.Expression == nil {
		return ""
	}

	// Check direct form: Layer.method(tag, impl)
	directMethod := tryGetLayerApiMethod(c, callExpr.Expression)
	if directMethod == method && callExpr.Arguments != nil && len(callExpr.Arguments.Nodes) >= 2 &&
		callExpr.Arguments.Nodes[1] == implementationExpr {
		return layerServiceNameFromExpression(callExpr.Arguments.Nodes[0])
	}

	// Check curried form: Layer.method(tag)(impl)
	if callExpr.Expression.Kind == ast.KindCallExpression {
		innerCall := callExpr.Expression.AsCallExpression()
		if innerCall != nil && innerCall.Expression != nil {
			innerMethod := tryGetLayerApiMethod(c, innerCall.Expression)
			if innerMethod == method && innerCall.Arguments != nil && len(innerCall.Arguments.Nodes) >= 1 &&
				callExpr.Arguments != nil && len(callExpr.Arguments.Nodes) >= 1 &&
				callExpr.Arguments.Nodes[0] == implementationExpr {
				return layerServiceNameFromExpression(innerCall.Arguments.Nodes[0])
			}
		}
	}

	return ""
}

// tryMatchLayerSucceedInference checks if an object literal is a direct argument to Layer.succeed.
// Returns the service tag text or empty string.
func tryMatchLayerSucceedInference(c *checker.Checker, objectLiteral *ast.Node) string {
	if objectLiteral == nil || objectLiteral.Parent == nil || objectLiteral.Parent.Kind != ast.KindCallExpression {
		return ""
	}
	callExpr := objectLiteral.Parent.AsCallExpression()
	if callExpr == nil {
		return ""
	}
	return verifyLayerMethodAtCall(c, callExpr, "succeed", objectLiteral)
}

// tryMatchLayerSyncInference checks if an object literal is returned from a lazy function
// that is passed to Layer.sync.
// Pattern: Layer.sync(tag)(() => { return { ... } }) or Layer.sync(tag, () => { return { ... } })
func tryMatchLayerSyncInference(c *checker.Checker, objectLiteral *ast.Node) string {
	if objectLiteral == nil || objectLiteral.Parent == nil {
		return ""
	}
	// objectLiteral -> ReturnStatement
	returnStmt := objectLiteral.Parent
	if returnStmt.Kind != ast.KindReturnStatement {
		return ""
	}
	// ReturnStatement -> Block
	block := returnStmt.Parent
	if block == nil || block.Kind != ast.KindBlock {
		return ""
	}
	// Block -> ArrowFunction or FunctionExpression (the lazy function)
	lazyFn := block.Parent
	if lazyFn == nil || (lazyFn.Kind != ast.KindArrowFunction && lazyFn.Kind != ast.KindFunctionExpression) {
		return ""
	}
	// LazyFunction -> CallExpression
	callNode := lazyFn.Parent
	if callNode == nil || callNode.Kind != ast.KindCallExpression {
		return ""
	}
	callExpr := callNode.AsCallExpression()
	if callExpr == nil {
		return ""
	}
	return verifyLayerMethodAtCall(c, callExpr, "sync", lazyFn)
}

// tryMatchLayerEffectInference checks if an object literal is returned from a generator
// inside Effect.gen that is passed to Layer.effect.
// Pattern: Layer.effect(tag)(Effect.gen(function*() { return { ... } }))
func tryMatchLayerEffectInference(c *checker.Checker, objectLiteral *ast.Node) string {
	if objectLiteral == nil || objectLiteral.Parent == nil {
		return ""
	}
	// objectLiteral -> ReturnStatement
	returnStmt := objectLiteral.Parent
	if returnStmt.Kind != ast.KindReturnStatement {
		return ""
	}
	// ReturnStatement -> Block (generator body)
	genBody := returnStmt.Parent
	if genBody == nil || genBody.Kind != ast.KindBlock {
		return ""
	}
	// Block -> FunctionExpression (generator function, must have asteriskToken)
	genFnNode := genBody.Parent
	if genFnNode == nil || genFnNode.Kind != ast.KindFunctionExpression {
		return ""
	}
	genFn := genFnNode.AsFunctionExpression()
	if genFn == nil || genFn.AsteriskToken == nil {
		return ""
	}
	// GeneratorFunction -> CallExpression (Effect.gen call)
	genCallNode := genFnNode.Parent
	if genCallNode == nil || genCallNode.Kind != ast.KindCallExpression {
		return ""
	}
	// Verify this is actually an Effect.gen call with our generator
	parsedGen := EffectGenCall(c, genCallNode)
	if parsedGen == nil || parsedGen.GeneratorFunction != genFn {
		return ""
	}
	// Effect.gen(...) -> CallExpression (Layer.effect call)
	layerCallNode := genCallNode.Parent
	if layerCallNode == nil || layerCallNode.Kind != ast.KindCallExpression {
		return ""
	}
	layerCall := layerCallNode.AsCallExpression()
	if layerCall == nil {
		return ""
	}
	return verifyLayerMethodAtCall(c, layerCall, "effect", genCallNode)
}

// tryGetLayerInferredTraceName checks if a function is inside a property assignment within
// an object literal that is a Layer service definition, returning "ServiceTag.memberName" format.
func tryGetLayerInferredTraceName(c *checker.Checker, node *ast.Node, suggestedTraceName string) string {
	if suggestedTraceName == "" || node == nil || node.Parent == nil {
		return ""
	}
	// The function must be the initializer of a PropertyAssignment
	if node.Parent.Kind != ast.KindPropertyAssignment {
		return ""
	}
	pa := node.Parent.AsPropertyAssignment()
	if pa == nil || pa.Initializer != node {
		return ""
	}
	// The PropertyAssignment must be inside an ObjectLiteralExpression
	if node.Parent.Parent == nil || node.Parent.Parent.Kind != ast.KindObjectLiteralExpression {
		return ""
	}
	objectLiteral := node.Parent.Parent

	// Try each Layer pattern in order
	if serviceName := tryMatchLayerSucceedInference(c, objectLiteral); serviceName != "" {
		return serviceName + "." + suggestedTraceName
	}
	if serviceName := tryMatchLayerSyncInference(c, objectLiteral); serviceName != "" {
		return serviceName + "." + suggestedTraceName
	}
	if serviceName := tryMatchLayerEffectInference(c, objectLiteral); serviceName != "" {
		return serviceName + "." + suggestedTraceName
	}
	if serviceName := tryMatchOfInference(c, objectLiteral); serviceName != "" {
		return serviceName + "." + suggestedTraceName
	}
	if serviceName := tryMatchServiceMapMakeInference(c, objectLiteral); serviceName != "" {
		return serviceName + "." + suggestedTraceName
	}
	return ""
}

// tryMatchOfInference checks if an object literal is the first argument to a Service.of({ ... }) call,
// where the service expression is a ContextTag or ServiceType.
// Returns the service tag text or empty string.
func tryMatchOfInference(c *checker.Checker, objectLiteral *ast.Node) string {
	if objectLiteral == nil || objectLiteral.Parent == nil || objectLiteral.Parent.Kind != ast.KindCallExpression {
		return ""
	}
	callExpr := objectLiteral.Parent.AsCallExpression()
	if callExpr == nil || callExpr.Arguments == nil || len(callExpr.Arguments.Nodes) < 1 {
		return ""
	}
	// objectLiteral must be the first argument
	if callExpr.Arguments.Nodes[0] != objectLiteral {
		return ""
	}
	// The call expression must be a PropertyAccessExpression with name "of"
	if callExpr.Expression == nil || callExpr.Expression.Kind != ast.KindPropertyAccessExpression {
		return ""
	}
	propAccess := callExpr.Expression.AsPropertyAccessExpression()
	if propAccess == nil || propAccess.Name() == nil {
		return ""
	}
	if scanner.GetTextOfNode(propAccess.Name()) != "of" {
		return ""
	}
	// Get the service tag expression (the object before .of)
	serviceTagExpression := propAccess.Expression
	if serviceTagExpression == nil {
		return ""
	}
	serviceTagType := checkerutils.GetTypeAtLocation(c,serviceTagExpression)
	if serviceTagType == nil {
		return ""
	}
	if !IsContextTag(c, serviceTagType, serviceTagExpression) && !IsServiceType(c, serviceTagType, serviceTagExpression) {
		return ""
	}
	return layerServiceNameFromExpression(serviceTagExpression)
}

// tryMatchServiceMapMakeInference checks if an object literal is returned from a generator
// inside Effect.gen that is the "make" property of a class extending ServiceMap.Service.
// Returns the class name or empty string.
func tryMatchServiceMapMakeInference(c *checker.Checker, objectLiteral *ast.Node) string {
	if objectLiteral == nil || objectLiteral.Parent == nil {
		return ""
	}
	// objectLiteral -> ReturnStatement
	returnStmt := objectLiteral.Parent
	if returnStmt.Kind != ast.KindReturnStatement {
		return ""
	}
	// ReturnStatement -> Block (generator body)
	genBody := returnStmt.Parent
	if genBody == nil || genBody.Kind != ast.KindBlock {
		return ""
	}
	// Block -> FunctionExpression (generator function, must have asteriskToken)
	genFnNode := genBody.Parent
	if genFnNode == nil || genFnNode.Kind != ast.KindFunctionExpression {
		return ""
	}
	genFn := genFnNode.AsFunctionExpression()
	if genFn == nil || genFn.AsteriskToken == nil {
		return ""
	}
	// GeneratorFunction -> CallExpression (Effect.gen call)
	genCallNode := genFnNode.Parent
	if genCallNode == nil || genCallNode.Kind != ast.KindCallExpression {
		return ""
	}
	// Verify this is actually an Effect.gen call with our generator
	parsedGen := EffectGenCall(c, genCallNode)
	if parsedGen == nil || parsedGen.GeneratorFunction != genFn {
		return ""
	}
	// Effect.gen(...) -> PropertyAssignment with name "make" and initializer == genCall
	makeProperty := genCallNode.Parent
	if makeProperty == nil || makeProperty.Kind != ast.KindPropertyAssignment {
		return ""
	}
	pa := makeProperty.AsPropertyAssignment()
	if pa == nil || pa.Initializer != genCallNode {
		return ""
	}
	if pa.Name() == nil || pa.Name().Kind != ast.KindIdentifier || scanner.GetTextOfNode(pa.Name()) != "make" {
		return ""
	}
	// Walk ancestors from PropertyAssignment.Parent to find a ClassDeclaration
	currentNode := makeProperty.Parent
	for currentNode != nil {
		if currentNode.Kind == ast.KindClassDeclaration {
			break
		}
		currentNode = currentNode.Parent
	}
	if currentNode == nil || currentNode.Name() == nil {
		return ""
	}
	// Verify the class extends ServiceMap.Service
	if ExtendsServiceMapService(c, currentNode) == nil {
		return ""
	}
	return scanner.GetTextOfNode(currentNode.Name())
}

// getInferredTraceName computes a context-aware inferred trace name for a function.
// It checks (in priority order):
// 1. Layer service context (ServiceTag.memberName)
// 2. Exported function declarations — returns the function name
// 3. Exported const variable initializers — returns the variable name
// Returns "" if no context-aware name can be inferred.
func getInferredTraceName(c *checker.Checker, node *ast.Node, suggestedTraceName string) string {
	if suggestedTraceName == "" {
		return ""
	}

	// Layer-based inferred trace name takes priority
	if inferredFromLayer := tryGetLayerInferredTraceName(c, node, suggestedTraceName); inferredFromLayer != "" {
		return inferredFromLayer
	}

	// Check exported function declaration
	if node.Kind == ast.KindFunctionDeclaration {
		if ast.HasSyntacticModifier(node, ast.ModifierFlagsExport) {
			return suggestedTraceName
		}
	}

	// Check exported const variable initializer
	if node.Parent != nil && node.Parent.Kind == ast.KindVariableDeclaration {
		vd := node.Parent.AsVariableDeclaration()
		if vd != nil && vd.Initializer == node && vd.Name() != nil && vd.Name().Kind == ast.KindIdentifier {
			// Walk up: VariableDeclaration -> VariableDeclarationList -> VariableStatement
			declList := node.Parent.Parent
			if declList != nil && declList.Kind == ast.KindVariableDeclarationList {
				varStmt := declList.Parent
				if varStmt != nil && varStmt.Kind == ast.KindVariableStatement {
					if ast.HasSyntacticModifier(varStmt, ast.ModifierFlagsExport) &&
						declList.Flags&ast.NodeFlagsConst != 0 {
						return suggestedTraceName
					}
				}
			}
		}
	}

	return ""
}
