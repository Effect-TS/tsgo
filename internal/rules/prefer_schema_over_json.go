// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// PreferSchemaOverJson detects JSON.parse and JSON.stringify call expressions inside
// Effect contexts (Effect.try or Effect.gen/Effect.fn) and suggests using Effect Schema instead.
// Both Effect v3 and v4 are supported (no version gating), except the simple Effect.try(() => ...)
// thunk form which is V3-only.
var PreferSchemaOverJson = rule.Rule{
	Name:        "preferSchemaOverJson",
	Description:     "Suggests using Effect Schema for JSON operations instead of JSON.parse/JSON.stringify",
	DefaultSeverity: etscore.SeveritySuggestion,
	Codes:       []int32{tsdiag.Consider_using_Effect_Schema_for_JSON_operations_instead_of_JSON_parse_SlashJSON_stringify_effect_preferSchemaOverJson.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic
		isV4 := typeparser.SupportedEffectVersion(ctx.Checker) == typeparser.EffectMajorV4

		var walk ast.Visitor
		walk = func(n *ast.Node) bool {
			if n == nil {
				return false
			}

			if n.Kind == ast.KindCallExpression {
				if d := checkPreferSchemaOverJson(ctx, n, isV4); d != nil {
					diags = append(diags, d)
				}
			}

			n.ForEachChild(walk)
			return false
		}

		walk(ctx.SourceFile.AsNode())
		return diags
	},
}

// checkPreferSchemaOverJson checks a single call expression for JSON.parse/stringify
// inside an Effect context (Effect.try or Effect.gen/Effect.fn).
func checkPreferSchemaOverJson(ctx *rule.Context, node *ast.Node, isV4 bool) *ast.Diagnostic {
	// Try each pattern in order
	if jsonNode := checkEffectTrySimple(ctx.Checker, node, isV4); jsonNode != nil {
		return ctx.NewDiagnostic(ctx.GetErrorRange(jsonNode), tsdiag.Consider_using_Effect_Schema_for_JSON_operations_instead_of_JSON_parse_SlashJSON_stringify_effect_preferSchemaOverJson, nil)
	}
	if jsonNode := checkEffectTryObject(ctx.Checker, node); jsonNode != nil {
		return ctx.NewDiagnostic(ctx.GetErrorRange(jsonNode), tsdiag.Consider_using_Effect_Schema_for_JSON_operations_instead_of_JSON_parse_SlashJSON_stringify_effect_preferSchemaOverJson, nil)
	}
	if jsonNode := checkJsonMethodInEffectGen(ctx.Checker, node); jsonNode != nil {
		return ctx.NewDiagnostic(ctx.GetErrorRange(jsonNode), tsdiag.Consider_using_Effect_Schema_for_JSON_operations_instead_of_JSON_parse_SlashJSON_stringify_effect_preferSchemaOverJson, nil)
	}
	return nil
}

// parseJsonMethod checks if a call expression is JSON.parse or JSON.stringify.
// Returns the call expression node if it matches, nil otherwise.
func parseJsonMethod(node *ast.Node) *ast.Node {
	if node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}

	call := node.AsCallExpression()
	if call == nil || call.Expression == nil {
		return nil
	}

	expr := call.Expression
	if expr.Kind != ast.KindPropertyAccessExpression {
		return nil
	}

	prop := expr.AsPropertyAccessExpression()
	if prop == nil || prop.Expression == nil || prop.Name() == nil {
		return nil
	}

	// Check that the object is an identifier "JSON"
	objectExpr := prop.Expression
	if objectExpr.Kind != ast.KindIdentifier {
		return nil
	}
	if scanner.GetTextOfNode(objectExpr) != "JSON" {
		return nil
	}

	// Check that the method is "parse" or "stringify"
	methodName := scanner.GetTextOfNode(prop.Name())
	if methodName != "parse" && methodName != "stringify" {
		return nil
	}

	return node
}

// checkEffectTrySimple matches Effect.try(() => JSON.parse/stringify(...)) - simple thunk form.
// This pattern is V3-only (the simple thunk form was removed in V4).
func checkEffectTrySimple(c *checker.Checker, node *ast.Node, isV4 bool) *ast.Node {
	if isV4 {
		return nil
	}

	if node.Kind != ast.KindCallExpression {
		return nil
	}
	call := node.AsCallExpression()

	// Check callee is Effect.try
	if !typeparser.IsNodeReferenceToEffectModuleApi(c, call.Expression, "try") {
		return nil
	}

	// Must have at least one argument
	if call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
		return nil
	}

	// Parse the first argument as a lazy expression (thunk=false, like the TS reference)
	lazyExpr := typeparser.ParseLazyExpression(call.Arguments.Nodes[0], false)
	if lazyExpr == nil {
		return nil
	}

	return parseJsonMethod(lazyExpr.Expression)
}

// checkEffectTryObject matches Effect.try({ try: () => JSON.parse/stringify(...), ... }) - object form.
func checkEffectTryObject(c *checker.Checker, node *ast.Node) *ast.Node {
	if node.Kind != ast.KindCallExpression {
		return nil
	}
	call := node.AsCallExpression()

	// Check callee is Effect.try
	if !typeparser.IsNodeReferenceToEffectModuleApi(c, call.Expression, "try") {
		return nil
	}

	// Must have at least one argument
	if call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
		return nil
	}

	// First argument must be an object literal
	arg := call.Arguments.Nodes[0]
	if arg == nil || arg.Kind != ast.KindObjectLiteralExpression {
		return nil
	}

	objLit := arg.AsObjectLiteralExpression()
	if objLit == nil || objLit.Properties == nil {
		return nil
	}

	// Find the "try" property
	var tryInitializer *ast.Node
	for _, prop := range objLit.Properties.Nodes {
		if prop == nil || prop.Kind != ast.KindPropertyAssignment {
			continue
		}
		pa := prop.AsPropertyAssignment()
		if pa == nil || pa.Name() == nil {
			continue
		}
		if pa.Name().Kind == ast.KindIdentifier && scanner.GetTextOfNode(pa.Name()) == "try" {
			tryInitializer = pa.Initializer
			break
		}
	}

	if tryInitializer == nil {
		return nil
	}

	// Parse the try property initializer as a lazy expression
	lazyExpr := typeparser.ParseLazyExpression(tryInitializer, false)
	if lazyExpr == nil {
		return nil
	}

	return parseJsonMethod(lazyExpr.Expression)
}

// checkJsonMethodInEffectGen matches direct JSON.parse/stringify inside an Effect generator (Effect.gen or Effect.fn).
func checkJsonMethodInEffectGen(c *checker.Checker, node *ast.Node) *ast.Node {
	// First check if this is a JSON method call
	jsonNode := parseJsonMethod(node)
	if jsonNode == nil {
		return nil
	}

	// Find enclosing scopes
	scopes := typeparser.FindEnclosingScopes(c, node)
	if scopes.ScopeKind != typeparser.ScopeKindEffectGen && scopes.ScopeKind != typeparser.ScopeKindEffectFn {
		return nil
	}

	genFn := scopes.EffectGeneratorFunction()
	if genFn == nil {
		return nil
	}

	// Check that the generator body has at least one statement
	if genFn.Body == nil || genFn.Body.Kind != ast.KindBlock {
		return nil
	}
	block := genFn.Body.AsBlock()
	if block.Statements == nil || len(block.Statements.Nodes) == 0 {
		return nil
	}

	// Must be in direct scope (not a nested function)
	if scopes.ScopeNode != nil && scopes.ScopeNode != genFn.AsNode() {
		return nil
	}

	return jsonNode
}
