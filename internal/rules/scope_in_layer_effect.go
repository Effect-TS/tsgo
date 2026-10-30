package rules

import (
	"slices"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// ScopeInLayerEffect suggests using Layer.scoped instead of Layer.effect when
// Scope is detected in the layer's requirements.
var ScopeInLayerEffect = rule.Rule{
	Name:            "scopeInLayerEffect",
	Group:           "antipattern",
	Description:     "Suggests using Layer.scoped instead of Layer.effect when Scope is in requirements",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v3"},
	Codes:           []int32{tsdiag.This_layer_construction_leaves_Scope_in_the_requirement_set_The_scoped_API_removes_Scope_from_the_resulting_requirements_effect_scopeInLayerEffect.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeScopeInLayerEffect(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.This_layer_construction_leaves_Scope_in_the_requirement_set_The_scoped_API_removes_Scope_from_the_resulting_requirements_effect_scopeInLayerEffect, nil)
		}
		return diags
	},
}

// ScopeInLayerEffectMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the scopeInLayerEffect pattern.
type ScopeInLayerEffectMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange // The pre-computed error range for this match
	Callee     *ast.Node      // The Layer constructor callee; nil for class declaration matches
}

// AnalyzeScopeInLayerEffect finds all Layer.effect*() steps and class declarations
// with Default layer properties where Scope is in the layer's requirements.
func AnalyzeScopeInLayerEffect(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []ScopeInLayerEffectMatch {
	// V3-only rule
	if tp.SupportedEffectVersion() != typeparser.EffectMajorV3 {
		return nil
	}

	var matches []ScopeInLayerEffectMatch

	// Pattern 1: Layer.effect*() steps, whether data-last, pipeable or curried
	for _, flow := range tp.PipingFlows(sf, false).All() {
		for index := range flow.Transformations().All() {
			transformation := flow.Transformations().At(index)
			if m := matchLayerEffectTransformation(tp, sf, &transformation); m != nil {
				matches = append(matches, *m)
			}
		}
	}

	// Pattern 2: Class declarations with Default layer property
	nodeToVisit := make([]*ast.Node, 0)
	pushChild := func(child *ast.Node) bool {
		nodeToVisit = append(nodeToVisit, child)
		return false
	}
	sf.AsNode().ForEachChild(pushChild)

	for len(nodeToVisit) > 0 {
		node := nodeToVisit[len(nodeToVisit)-1]
		nodeToVisit = nodeToVisit[:len(nodeToVisit)-1]

		if node.Kind == ast.KindClassDeclaration {
			if m := matchClassWithDefaultLayer(tp, c, sf, node); m != nil {
				matches = append(matches, *m)
				continue // skip children
			}
		}

		node.ForEachChild(pushChild)
	}

	return matches
}

// matchLayerEffectTransformation checks if a piping flow step applies one of the
// Layer.effect* constructors and produces a layer with Scope in RIn.
func matchLayerEffectTransformation(tp *typeparser.TypeParser, sf *ast.SourceFile, transformation *typeparser.PipingFlowTransformation) *ScopeInLayerEffectMatch {
	callee := transformation.Callee()
	if callee == nil || callee.Parent == nil || !ast.IsCallExpression(callee.Parent) || callee.Parent.AsCallExpression().Expression != callee {
		return nil
	}

	// Verify this references one of the Layer.effect* constructors from the
	// "effect" package. Symbol resolution also recognizes named imports and
	// stable local aliases whose source spelling does not reveal the API name.
	if !slices.ContainsFunc([]string{"effect", "effectDiscard", "effectContext"}, func(candidate string) bool {
		return tp.IsNodeReferenceToEffectLayerModuleApi(callee, candidate)
	}) {
		return nil
	}

	layer := tp.LayerType(transformation.OutType())
	if layer == nil || !hasScope(tp, layer.RIn()) {
		return nil
	}

	// Report the call that produces the layer: the outermost application of
	// a curried Layer.effect(tag)(effect), or the constructor call itself.
	call := callee.Parent
	for call.Parent != nil && ast.IsCallExpression(call.Parent) && call.Parent.AsCallExpression().Expression == call {
		call = call.Parent
	}

	return &ScopeInLayerEffectMatch{
		SourceFile: sf,
		Location:   scanner.GetErrorRangeForNode(sf, call),
		Callee:     callee,
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
	classSym := tp.GetSymbolAtLocation(node.Name())
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
	layer := tp.LayerType(defaultType)
	if layer == nil {
		return nil
	}

	// Check if RIn contains a Scope type
	if !hasScope(tp, layer.RIn()) {
		return nil
	}

	return &ScopeInLayerEffectMatch{
		SourceFile: sf,
		Location:   scanner.GetErrorRangeForNode(sf, node),
	}
}

// hasScope checks if any union member of the given type is a Scope type.
func hasScope(tp *typeparser.TypeParser, t *checker.Type) bool {
	members := tp.UnrollUnionMembers(t)
	return slices.ContainsFunc(members, func(member *checker.Type) bool {
		return tp.ScopeType(member) != nil
	})
}
