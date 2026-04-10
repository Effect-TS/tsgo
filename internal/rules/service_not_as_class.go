package rules

import (
	"fmt"
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

var ServiceNotAsClass = rule.Rule{
	Name:            "serviceNotAsClass",
	Group:           "style",
	Description:     "Warns when Context.Service is used as a variable instead of a class declaration",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.Context_Service_is_assigned_to_a_variable_here_but_this_API_is_intended_for_a_class_declaration_shape_such_as_0_effect_serviceNotAsClass.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeServiceNotAsClass(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.Context_Service_is_assigned_to_a_variable_here_but_this_API_is_intended_for_a_class_declaration_shape_such_as_0_effect_serviceNotAsClass, nil, m.SuggestedUsage)
		}
		return diags
	},
}

// ServiceNotAsClassMatch holds the data needed by both the diagnostic and the quickfix.
type ServiceNotAsClassMatch struct {
	SourceFile     *ast.SourceFile
	Location       core.TextRange    // Error range for the call expression
	SuggestedUsage string            // The full suggested class declaration string for the diagnostic message
	CallExprNode   *ast.Node         // The call expression node (ServiceMap.Service<...>(...) or Context.Service<...>(...))
	VariableName   string            // The variable/class name
	TypeArgsText   string            // Text of original type arguments (e.g. "ConfigService")
	ArgsText       string            // Text of original call arguments (e.g. `"Config"`)
	ServiceModule  string            // Service namespace identifier (e.g. "ServiceMap" or "Context")
	TargetNode     *ast.Node         // The node to replace (variable statement or declaration list)
	ModifierNodes  *ast.ModifierList // Modifiers from the variable statement (e.g. export)
}

// AnalyzeServiceNotAsClass finds all const variable declarations using the v4 service constructor
// that should be class declarations instead. V4-only rule.
func AnalyzeServiceNotAsClass(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []ServiceNotAsClassMatch {
	if tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	var matches []ServiceNotAsClassMatch

	nodeToVisit := make([]*ast.Node, 0)
	pushChild := func(child *ast.Node) bool {
		nodeToVisit = append(nodeToVisit, child)
		return false
	}
	sf.AsNode().ForEachChild(pushChild)

	for len(nodeToVisit) > 0 {
		node := nodeToVisit[len(nodeToVisit)-1]
		nodeToVisit = nodeToVisit[:len(nodeToVisit)-1]

		if node.Kind == ast.KindVariableDeclaration {
			if m := checkServiceNotAsClass(tp, c, sf, node); m != nil {
				matches = append(matches, *m)
			}
		}

		node.ForEachChild(pushChild)
	}

	return matches
}

func checkServiceNotAsClass(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile, node *ast.Node) *ServiceNotAsClassMatch {
	varDecl := node.AsVariableDeclaration()
	if varDecl == nil || varDecl.Initializer == nil {
		return nil
	}

	if varDecl.Initializer.Kind != ast.KindCallExpression {
		return nil
	}

	callExpr := varDecl.Initializer.AsCallExpression()
	if callExpr.TypeArguments == nil || len(callExpr.TypeArguments.Nodes) == 0 {
		return nil
	}

	// Check parent is a const declaration list
	declList := node.Parent
	if declList == nil || declList.Kind != ast.KindVariableDeclarationList {
		return nil
	}
	if declList.Flags&ast.NodeFlagsConst == 0 {
		return nil
	}

	serviceModule := ""
	if tp.IsNodeReferenceToEffectContextModuleApi(callExpr.Expression, "Service") {
		serviceModule = "Context"
	}
	if serviceModule == "" {
		return nil
	}

	text := sf.Text()

	// Extract variable name
	variableName := extractNodeText(sf, text, node.Name())

	// Extract type arguments text
	typeArgs := callExpr.TypeArguments.Nodes
	typeArgTexts := make([]string, len(typeArgs))
	for i, ta := range typeArgs {
		typeArgTexts[i] = extractNodeText(sf, text, ta)
	}
	typeArgsText := strings.Join(typeArgTexts, ", ")

	// Extract call arguments text
	var argsText string
	if len(callExpr.Arguments.Nodes) > 0 {
		argTexts := make([]string, len(callExpr.Arguments.Nodes))
		for i, arg := range callExpr.Arguments.Nodes {
			argTexts[i] = extractNodeText(sf, text, arg)
		}
		argsText = strings.Join(argTexts, ", ")
	}

	// Build suggested usage string using the matched service namespace.
	var suggestedUsage string
	if argsText != "" {
		suggestedUsage = fmt.Sprintf("class %s extends %s.Service<%s, %s>()(%s) {}", variableName, serviceModule, variableName, typeArgsText, argsText)
	} else {
		suggestedUsage = fmt.Sprintf("class %s extends %s.Service<%s, %s>() {}", variableName, serviceModule, variableName, typeArgsText)
	}

	// Determine target node and modifiers
	variableStatement := declList.Parent
	var targetNode *ast.Node
	var modifierNodes *ast.ModifierList
	if variableStatement != nil && variableStatement.Kind == ast.KindVariableStatement {
		targetNode = variableStatement
		modifierNodes = variableStatement.Modifiers()
	} else {
		targetNode = declList
	}

	return &ServiceNotAsClassMatch{
		SourceFile:     sf,
		Location:       scanner.GetErrorRangeForNode(sf, varDecl.Initializer),
		SuggestedUsage: suggestedUsage,
		CallExprNode:   varDecl.Initializer,
		VariableName:   variableName,
		TypeArgsText:   typeArgsText,
		ArgsText:       argsText,
		ServiceModule:  serviceModule,
		TargetNode:     targetNode,
		ModifierNodes:  modifierNodes,
	}
}

// extractNodeText gets the source text of a node, skipping leading trivia.
func extractNodeText(sf *ast.SourceFile, text string, node *ast.Node) string {
	if node == nil {
		return ""
	}
	start := scanner.GetTokenPosOfNode(node, sf, false)
	end := node.End()
	if start >= 0 && end >= start && end <= len(text) {
		return text[start:end]
	}
	return ""
}
