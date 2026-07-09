package fixables

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/effect-ts/tsgo/internal/rewriter"
)

var EffectFnOpportunityFix = fixable.Fixable{
	Name:        "effectFnOpportunity",
	Description: "Convert to Effect.fn",
	ErrorCodes:  []int32{tsdiag.This_expression_can_be_rewritten_in_the_reusable_function_form_0_effect_effectFnOpportunity.Code()},
	FixIDs: []string{
		"effectFnOpportunity_toEffectFnWithSpan",
		"effectFnOpportunity_toEffectFnUntraced",
		"effectFnOpportunity_toEffectFnNoSpan",
		"effectFnOpportunity_toEffectFnSpanInferred",
		"effectFnOpportunity_toEffectFnSpanSuggested",
	},
	Run: runEffectFnOpportunityFix,
}

func runEffectFnOpportunityFix(ctx *fixable.Context) []ls.CodeAction {

	c := ctx.Checker

	sf := ctx.SourceFile

	effectConfig := ctx.Options

	matches := rules.AnalyzeEffectFnOpportunity(ctx.TypeParser, c, sf)

	var result *typeparser.EffectFnOpportunityResult
	for _, match := range matches {
		diagRange := match.Location
		if diagRange.Intersects(ctx.Span) || ctx.Span.ContainedBy(diagRange) {
			result = match.Result
			break
		}
	}
	if result == nil {
		return nil
	}

	isFuncDecl := result.TargetNode.Kind == ast.KindFunctionDeclaration

	var actions []ls.CodeAction

	// Fix 1: toEffectFnWithSpan - available when explicit withSpan expression exists
	if effectConfig.EffectFnIncludes(etscore.EffectFnSpan) && result.ExplicitTraceExpression != nil {
		// Remove withSpan from pipe args (it's the last one)
		pipeArgs := result.PipeArguments
		if len(pipeArgs) > 0 {
			pipeArgs = pipeArgs[:len(pipeArgs)-1]
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Convert to Effect.fn (with span from withSpan)",
			Run: func(tracker *rewriter.Tracker) {
				traceNode := tracker.DeepCloneNode(result.ExplicitTraceExpression)
				effectFnBuildReplacement(tracker, sf, result, "fn", traceNode, pipeArgs, isFuncDecl)
			},
		}); action != nil {
			actions = append(actions, *action)
		}
	}

	// Fix 2: toEffectFnUntraced - available when gen opportunity (generator function exists)
	if effectConfig.EffectFnIncludes(etscore.EffectFnUntraced) && result.GeneratorFunction != nil {
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Convert to Effect.fnUntraced",
			Run: func(tracker *rewriter.Tracker) {
				effectFnBuildReplacement(tracker, sf, result, "fnUntraced", nil, result.PipeArguments, isFuncDecl)
			},
		}); action != nil {
			actions = append(actions, *action)
		}
	}

	// Fix 3: toEffectFnNoSpan - available when no-span variant is enabled
	if effectConfig.EffectFnIncludes(etscore.EffectFnNoSpan) {
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Convert to Effect.fn (no span)",
			Run: func(tracker *rewriter.Tracker) {
				effectFnBuildReplacement(tracker, sf, result, "fn", nil, result.PipeArguments, isFuncDecl)
			},
		}); action != nil {
			actions = append(actions, *action)
		}
	}

	// Fix 4: toEffectFnSpanInferred - available when no explicit withSpan and inferred trace name exists
	if effectConfig.EffectFnIncludes(etscore.EffectFnInferredSpan) && result.ExplicitTraceExpression == nil && result.InferredTraceName != "" {
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Convert to Effect.fn(\"" + result.InferredTraceName + "\")",
			Run: func(tracker *rewriter.Tracker) {
				traceNode := tracker.NewStringLiteral(result.InferredTraceName, 0)
				effectFnBuildReplacement(tracker, sf, result, "fn", traceNode, result.PipeArguments, isFuncDecl)
			},
		}); action != nil {
			actions = append(actions, *action)
		}
	}

	// Fix 5: toEffectFnSpanSuggested - available when no explicit withSpan, has suggested trace name,
	// and (inferred-span is not enabled OR suggested name differs from inferred name)
	if effectConfig.EffectFnIncludes(etscore.EffectFnSuggestedSpan) && result.ExplicitTraceExpression == nil && result.SuggestedTraceName != "" &&
		(!effectConfig.EffectFnIncludes(etscore.EffectFnInferredSpan) || result.SuggestedTraceName != result.InferredTraceName) {
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Convert to Effect.fn(\"" + result.SuggestedTraceName + "\")",
			Run: func(tracker *rewriter.Tracker) {
				traceNode := tracker.NewStringLiteral(result.SuggestedTraceName, 0)
				effectFnBuildReplacement(tracker, sf, result, "fn", traceNode, result.PipeArguments, isFuncDecl)
			},
		}); action != nil {
			actions = append(actions, *action)
		}
	}

	return actions
}

// effectFnBuildReplacement builds the complete Effect.fn/fnUntraced replacement AST node
// and applies it via tracker.ReplaceNode.
func effectFnBuildReplacement(
	tracker *rewriter.Tracker,
	sf *ast.SourceFile,
	result *typeparser.EffectFnOpportunityResult,
	variant string,
	traceNode *ast.Node,
	pipeArgs []*ast.Node,
	isFuncDecl bool,
) {
	// Build Effect module identifier
	var effectModuleId *ast.Node
	if result.EffectModule != nil && result.EffectModule.Kind == ast.KindIdentifier {
		effectModuleId = tracker.DeepCloneNode(result.EffectModule)
	} else {
		effectModuleId = tracker.NewIdentifier("Effect")
	}

	// Build inner body function
	var bodyFn *ast.Node
	if result.HasGenBody && result.GeneratorFunction != nil {
		bodyFn = effectFnBuildGenBody(tracker, result)
	} else {
		bodyFn = effectFnBuildRegularBody(tracker, result)
	}
	if bodyFn == nil {
		return
	}

	// Build Effect.fn/fnUntraced property access
	fnAccess := tracker.NewPropertyAccessExpression(effectModuleId, nil, tracker.NewIdentifier(variant), ast.NodeFlagsNone)

	// Collect inner args: body function + deep-cloned pipe args
	innerArgs := make([]*ast.Node, 0, 1+len(pipeArgs))
	innerArgs = append(innerArgs, bodyFn)
	for _, arg := range pipeArgs {
		innerArgs = append(innerArgs, tracker.DeepCloneNode(arg))
	}

	var callExpr *ast.Node
	if traceNode != nil {
		// Curried form: Effect.fn(traceNode)(bodyFn, ...pipeArgs)
		outerCall := tracker.NewCallExpression(fnAccess, nil, nil, tracker.NewNodeList([]*ast.Node{traceNode}), ast.NodeFlagsNone)
		callExpr = tracker.NewCallExpression(outerCall, nil, nil, tracker.NewNodeList(innerArgs), ast.NodeFlagsNone)
	} else {
		// Direct form: Effect.fn(bodyFn, ...pipeArgs) or Effect.fnUntraced(bodyFn, ...pipeArgs)
		callExpr = tracker.NewCallExpression(fnAccess, nil, nil, tracker.NewNodeList(innerArgs), ast.NodeFlagsNone)
	}

	// Determine replacement node and target node to replace.
	// For function declarations, replace the declaration with a variable statement.
	// For Layer member functions, replace only the property value (TargetNode), not the
	// enclosing variable statement, to preserve the surrounding Layer construction.
	// For other expressions (arrow functions, function expressions), replace the enclosing
	// variable statement if one exists. This ensures the replacement is always a
	// statement-level node, which avoids a formatter panic in the TypeScript-Go
	// format.processChildNode assertion when the replacement tree contains embedded
	// statements (e.g., if-then branches) and is formatted as a non-statement node.
	var replacementNode *ast.Node
	var replaceTarget *ast.Node
	if isFuncDecl {
		replacementNode = effectFnBuildVarStatement(tracker, result.TargetNode, callExpr)
		replaceTarget = result.TargetNode
	} else if result.IsLayerMember {
		// Layer member: replace the enclosing `return { ... }` statement instead of
		// just the property value. Formatting the larger statement-level replacement
		// avoids formatter assertions on synthetic embedded statements in function bodies.
		if returnStmt := effectFnFindEnclosingReturnedObject(result.TargetNode); returnStmt != nil {
			replacementNode = effectFnBuildLayerMemberReturnStatement(tracker, returnStmt, result.TargetNode, callExpr)
			replaceTarget = returnStmt
		} else {
			replacementNode = callExpr
			replaceTarget = result.TargetNode
		}
	} else if varStmt := effectFnFindEnclosingVarStatement(result.TargetNode); varStmt != nil {
		replacementNode = effectFnBuildVarStatementFromEnclosing(tracker, varStmt, result.TargetNode, callExpr)
		replaceTarget = varStmt
	} else {
		replacementNode = callExpr
		replaceTarget = result.TargetNode
	}

	ast.SetParentInChildren(replacementNode)
	tracker.ReplaceNode(sf, replaceTarget, replacementNode, nil)
}

// effectFnBuildGenBody builds a generator function expression for a gen opportunity:
// function*<TypeParams>(params) { ...generatorBody... }
func effectFnBuildGenBody(tracker *rewriter.Tracker, result *typeparser.EffectFnOpportunityResult) *ast.Node {
	genFn := result.GeneratorFunction
	if genFn == nil || genFn.Body == nil {
		return nil
	}

	typeParams := effectFnCloneNodeList(tracker, typeparser.GetFunctionLikeTypeParameters(result.TargetNode))
	params := effectFnCloneNodeList(tracker, typeparser.GetFunctionLikeParameters(result.TargetNode))
	body := tracker.DeepCloneNode(genFn.Body)

	return tracker.NewFunctionExpression(
		nil,                                     // modifiers
		tracker.NewToken(ast.KindAsteriskToken), // asteriskToken
		nil,                                     // name (anonymous)
		typeParams,                              // typeParameters
		params,                                  // parameters
		nil,                                     // returnType
		nil,                                     // fullSignature
		body,                                    // body
	)
}

// effectFnBuildRegularBody builds the body function for a regular (non-gen) opportunity.
// For function declarations and arrow functions, creates an anonymous function expression.
// For function expressions, deep-clones the entire node.
func effectFnBuildRegularBody(tracker *rewriter.Tracker, result *typeparser.EffectFnOpportunityResult) *ast.Node {
	if result.TargetNode.Kind == ast.KindFunctionDeclaration {
		fd := result.TargetNode.AsFunctionDeclaration()
		if fd == nil || fd.Body == nil {
			return nil
		}

		typeParams := effectFnCloneNodeList(tracker, fd.TypeParameters)
		params := effectFnCloneNodeList(tracker, fd.Parameters)
		body := tracker.DeepCloneNode(fd.Body)

		return tracker.NewFunctionExpression(
			nil,        // modifiers
			nil,        // asteriskToken (no generator)
			nil,        // name (anonymous)
			typeParams, // typeParameters
			params,     // parameters
			nil,        // returnType
			nil,        // fullSignature
			body,       // body
		)
	}

	// For arrow functions, convert to an anonymous function expression to match upstream behavior.
	if result.TargetNode.Kind == ast.KindArrowFunction {
		af := result.TargetNode.AsArrowFunction()
		if af == nil || af.Body == nil {
			return nil
		}

		typeParams := effectFnCloneNodeList(tracker, af.TypeParameters)
		params := effectFnCloneNodeList(tracker, af.Parameters)

		// If the arrow has an expression body, wrap it in a block with a return statement.
		var body *ast.Node
		if af.Body.Kind == ast.KindBlock {
			body = tracker.DeepCloneNode(af.Body)
		} else {
			returnStmt := tracker.NewReturnStatement(tracker.DeepCloneNode(af.Body))
			body = tracker.NewBlock(tracker.NewNodeList([]*ast.Node{returnStmt}), true)
		}

		return tracker.NewFunctionExpression(
			nil,        // modifiers
			nil,        // asteriskToken (no generator)
			nil,        // name (anonymous)
			typeParams, // typeParameters
			params,     // parameters
			nil,        // returnType
			nil,        // fullSignature
			body,       // body
		)
	}

	// For function expressions, deep-clone the entire target node
	return tracker.DeepCloneNode(result.TargetNode)
}

// effectFnBuildVarStatement wraps a call expression in a variable statement:
// [export] const name = callExpr
func effectFnBuildVarStatement(tracker *rewriter.Tracker, fnNode *ast.Node, callExpr *ast.Node) *ast.Node {
	fd := fnNode.AsFunctionDeclaration()
	if fd == nil {
		return callExpr
	}

	name := fd.Name()
	if name == nil {
		return callExpr
	}

	// Build variable declaration: const name = callExpr
	varDecl := tracker.NewVariableDeclaration(tracker.DeepCloneNode(name), nil, nil, callExpr)
	varDeclList := tracker.NewVariableDeclarationList(tracker.NewNodeList([]*ast.Node{varDecl}), ast.NodeFlagsConst)

	// Build modifier list (export etc.), excluding async
	var modifierList *ast.ModifierList
	if modifiers := fd.Modifiers(); modifiers != nil {
		var modNodes []*ast.Node
		for _, mod := range modifiers.Nodes {
			if mod.Kind == ast.KindAsyncKeyword {
				continue
			}
			modNodes = append(modNodes, tracker.NewModifier(mod.Kind))
		}
		if len(modNodes) > 0 {
			modifierList = tracker.NewModifierList(modNodes)
		}
	}

	return tracker.NewVariableStatement(modifierList, varDeclList)
}

// effectFnCloneNodeList deep-clones all nodes in a NodeList, returning a new synthesized NodeList.
func effectFnCloneNodeList(tracker *rewriter.Tracker, list *ast.NodeList) *ast.NodeList {
	if list == nil || len(list.Nodes) == 0 {
		return nil
	}
	cloned := make([]*ast.Node, len(list.Nodes))
	for i, node := range list.Nodes {
		cloned[i] = tracker.DeepCloneNode(node)
	}
	return tracker.NewNodeList(cloned)
}

// effectFnFindEnclosingVarStatement navigates up from a target node (which should be
// the initializer of a VariableDeclaration) to find the enclosing VariableStatement.
// Returns nil if the expected parent chain is not found.
func effectFnFindEnclosingVarStatement(node *ast.Node) *ast.Node {
	return ast.FindAncestorKind(node, ast.KindVariableStatement)
}

// effectFnFindEnclosingReturnedObject returns the enclosing ReturnStatement when
// the target node is the initializer of a PropertyAssignment inside a returned
// object literal, such as a Layer service member.
func effectFnFindEnclosingReturnedObject(node *ast.Node) *ast.Node {
	propAssign := ast.FindAncestorKind(node, ast.KindPropertyAssignment)
	if propAssign == nil {
		return nil
	}

	objLiteral := propAssign.Parent
	if objLiteral == nil || objLiteral.Kind != ast.KindObjectLiteralExpression {
		return nil
	}

	returnStmt := ast.FindAncestorKind(objLiteral, ast.KindReturnStatement)
	if returnStmt == nil {
		return nil
	}

	rs := returnStmt.AsReturnStatement()
	if rs == nil || rs.Expression != objLiteral {
		return nil
	}

	return returnStmt
}

// effectFnBuildLayerMemberReturnStatement rebuilds a `return { ... }` statement,
// replacing the target property's initializer with the synthesized Effect.fn call.
func effectFnBuildLayerMemberReturnStatement(tracker *rewriter.Tracker, returnStmt *ast.Node, targetNode *ast.Node, callExpr *ast.Node) *ast.Node {
	rs := returnStmt.AsReturnStatement()
	if rs == nil || rs.Expression == nil || rs.Expression.Kind != ast.KindObjectLiteralExpression {
		return callExpr
	}

	targetProp := ast.FindAncestorKind(targetNode, ast.KindPropertyAssignment)
	if targetProp == nil {
		return callExpr
	}

	objLiteral := rs.Expression.AsObjectLiteralExpression()
	if objLiteral == nil || objLiteral.Properties == nil {
		return callExpr
	}

	properties := make([]*ast.Node, len(objLiteral.Properties.Nodes))
	for i, prop := range objLiteral.Properties.Nodes {
		if prop != targetProp {
			properties[i] = tracker.DeepCloneNode(prop)
			continue
		}

		pa := prop.AsPropertyAssignment()
		if pa == nil {
			properties[i] = tracker.DeepCloneNode(prop)
			continue
		}

		var typeNode *ast.Node
		if pa.Type != nil {
			typeNode = tracker.DeepCloneNode(pa.Type)
		}

		properties[i] = tracker.NewPropertyAssignment(
			nil,
			tracker.DeepCloneNode(pa.Name()),
			nil,
			typeNode,
			callExpr,
		)
	}

	newObjLiteral := tracker.NewObjectLiteralExpression(tracker.NewNodeList(properties), true)
	return tracker.NewReturnStatement(newObjLiteral)
}

// effectFnBuildVarStatementFromEnclosing builds a replacement VariableStatement from an
// existing one, replacing the target node's initializer with callExpr.
// This preserves the variable name, type annotation, modifiers, and const/let/var flag.
func effectFnBuildVarStatementFromEnclosing(tracker *rewriter.Tracker, varStmt *ast.Node, targetNode *ast.Node, callExpr *ast.Node) *ast.Node {
	vs := varStmt.AsVariableStatement()
	if vs == nil {
		return callExpr
	}
	declList := vs.DeclarationList.AsVariableDeclarationList()
	if declList == nil || declList.Declarations == nil || len(declList.Declarations.Nodes) == 0 {
		return callExpr
	}

	// Get the variable declaration containing the target
	varDeclNode := ast.FindAncestorKind(targetNode, ast.KindVariableDeclaration)
	if varDeclNode == nil {
		return callExpr
	}
	varDecl := varDeclNode.AsVariableDeclaration()
	if varDecl == nil {
		return callExpr
	}
	name := varDecl.Name()
	if name == nil {
		return callExpr
	}

	// Build new variable declaration: preserve name and type, use callExpr as initializer
	var typeNode *ast.Node
	if varDecl.Type != nil {
		typeNode = tracker.DeepCloneNode(varDecl.Type)
	}
	newDecl := tracker.NewVariableDeclaration(tracker.DeepCloneNode(name), nil, typeNode, callExpr)

	// Preserve const/let/var flags from the original declaration list node
	flags := vs.DeclarationList.Flags & (ast.NodeFlagsConst | ast.NodeFlagsLet | ast.NodeFlagsUsing | ast.NodeFlagsAwaitUsing)
	newDeclList := tracker.NewVariableDeclarationList(tracker.NewNodeList([]*ast.Node{newDecl}), flags)

	// Build modifier list (export, declare, etc.)
	var modifierList *ast.ModifierList
	if modifiers := vs.Modifiers(); modifiers != nil {
		var modNodes []*ast.Node
		for _, mod := range modifiers.Nodes {
			modNodes = append(modNodes, tracker.NewModifier(mod.Kind))
		}
		if len(modNodes) > 0 {
			modifierList = tracker.NewModifierList(modNodes)
		}
	}

	return tracker.NewVariableStatement(modifierList, newDeclList)
}
