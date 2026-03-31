package rules

import (
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// ScopeInLayerEffect suggests using Layer.scoped instead of Layer.effect when
// Scope is detected in the layer's requirements.
var ScopeInLayerEffect = rule.Rule{
	Name:            "scopeInLayerEffect",
	Group:           "antipattern",
	Description:     "Suggests using Layer.scoped instead of Layer.effect when Scope is in requirements",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v3"},
	Codes:           []int32{tsdiag.Seems_like_you_are_constructing_a_layer_with_a_scope_in_the_requirements_Consider_using_scoped_instead_to_get_rid_of_the_scope_in_the_requirements_effect_scopeInLayerEffect.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeScopeInLayerEffect(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.Seems_like_you_are_constructing_a_layer_with_a_scope_in_the_requirements_Consider_using_scoped_instead_to_get_rid_of_the_scope_in_the_requirements_effect_scopeInLayerEffect, nil)
		}
		return diags
	},
}

// ScopeInLayerEffectMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the scopeInLayerEffect pattern.
type ScopeInLayerEffectMatch struct {
	SourceFile       *ast.SourceFile
	Location         core.TextRange // The pre-computed error range for this match
	MethodIdentifier *ast.Node      // The property name identifier node (e.g., "effect" in Layer.effect); nil for class declaration matches
}

// AnalyzeScopeInLayerEffect finds all Layer.effect*() calls and class declarations
// with Default layer properties where Scope is in the layer's requirements.
func AnalyzeScopeInLayerEffect(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []ScopeInLayerEffectMatch {
	// V3-only rule
	if tp.SupportedEffectVersion() != typeparser.EffectMajorV3 {
		return nil
	}

	var matches []ScopeInLayerEffectMatch

	// Stack-based traversal
	nodeToVisit := make([]*ast.Node, 0)
	pushChild := func(child *ast.Node) bool {
		nodeToVisit = append(nodeToVisit, child)
		return false
	}
	sf.AsNode().ForEachChild(pushChild)

	for len(nodeToVisit) > 0 {
		node := nodeToVisit[len(nodeToVisit)-1]
		nodeToVisit = nodeToVisit[:len(nodeToVisit)-1]

		// Pattern 1: Layer.effect*() calls
		if node.Kind == ast.KindCallExpression {
			if m := matchLayerEffectCall(tp, c, sf, node); m != nil {
				matches = append(matches, *m)
				continue // skip children
			}
		}

		// Pattern 2: Class declarations with Default layer property
		if node.Kind == ast.KindClassDeclaration {
			if m := matchClassWithDefaultLayer(tp, c, sf, node); m != nil {
				matches = append(matches, *m)
				continue // skip children
			}
		}

		// Enqueue children
		node.ForEachChild(pushChild)
	}

	return matches
}

// matchLayerEffectCall checks if a call expression is Layer.effect*() with Scope in RIn.
func matchLayerEffectCall(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, node *ast.Node) *ScopeInLayerEffectMatch {
	if node.Kind != ast.KindCallExpression {
		return nil
	}
	call := node.AsCallExpression()
	if call.Expression == nil || call.Expression.Kind != ast.KindPropertyAccessExpression {
		return nil
	}

	propAccess := call.Expression.AsPropertyAccessExpression()
	if propAccess.Name() == nil {
		return nil
	}

	// Check the method name starts with "effect" (case-insensitive)
	methodName := scanner.GetTextOfNode(propAccess.Name())
	if !strings.HasPrefix(strings.ToLower(methodName), "effect") {
		return nil
	}

	// Verify this references the Layer module from the "effect" package
	if !tp.IsNodeReferenceToEffectLayerModuleApi(call.Expression, methodName) {
		return nil
	}

	// Get the return type of the call
	t := tp.GetTypeAtLocation(node)
	if t == nil {
		return nil
	}

	// Parse as Layer type
	layer := tp.LayerType(t, node)
	if layer == nil {
		return nil
	}

	// Check if RIn contains a Scope type
	if !hasScope(tp, c, layer.RIn, node) {
		return nil
	}

	return &ScopeInLayerEffectMatch{
		SourceFile:       sf,
		Location:         scanner.GetErrorRangeForNode(sf, node),
		MethodIdentifier: propAccess.Name(),
	}
}

// matchClassWithDefaultLayer checks if a class declaration has a Default layer property with Scope in RIn.
func matchClassWithDefaultLayer(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, node *ast.Node) *ScopeInLayerEffectMatch {
	if node.Kind != ast.KindClassDeclaration {
		return nil
	}
	classDecl := node.AsClassDeclaration()

	// Must have a name and heritage clauses
	if node.Name() == nil || classDecl.HeritageClauses == nil {
		return nil
	}

	// Get the class symbol
	classSym := c.GetSymbolAtLocation(node.Name())
	if classSym == nil {
		return nil
	}

	// Get the class type
	classType := c.GetTypeOfSymbolAtLocation(classSym, node)
	if classType == nil {
		return nil
	}

	// Check for a "Default" property
	defaultProp := c.GetPropertyOfType(classType, "Default")
	if defaultProp == nil {
		return nil
	}

	// Get the Default property's type
	defaultType := c.GetTypeOfSymbolAtLocation(defaultProp, node)
	if defaultType == nil {
		return nil
	}

	// Parse as Layer type
	layer := tp.LayerType(defaultType, node)
	if layer == nil {
		return nil
	}

	// Check if RIn contains a Scope type
	if !hasScope(tp, c, layer.RIn, node) {
		return nil
	}

	return &ScopeInLayerEffectMatch{
		SourceFile:       sf,
		Location:         scanner.GetErrorRangeForNode(sf, node),
		MethodIdentifier: nil,
	}
}

// hasScope checks if any union member of the given type is a Scope type.
func hasScope(tp *typeparser.TypeParser, _ *checker.Checker, t *checker.Type, atLocation *ast.Node) bool {
	members := typeparser.UnrollUnionMembers(t)
	for _, member := range members {
		if tp.IsScopeType(member, atLocation) {
			return true
		}
	}
	return false
}
