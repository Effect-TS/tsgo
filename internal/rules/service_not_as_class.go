package rules

import (
	"fmt"
	"strings"

	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var ServiceNotAsClass = rule.Rule{
	Name:            "serviceNotAsClass",
	Group:           "style",
	Description:     "Warns when ServiceMap.Service is used as a variable instead of a class declaration",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.ServiceMap_Service_should_be_used_in_a_class_declaration_instead_of_as_a_variable_Use_Colon_0_effect_serviceNotAsClass.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeServiceNotAsClass(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.ServiceMap_Service_should_be_used_in_a_class_declaration_instead_of_as_a_variable_Use_Colon_0_effect_serviceNotAsClass, nil, m.SuggestedUsage)
		}
		return diags
	},
}

// ServiceNotAsClassMatch holds the data needed by both the diagnostic and the quickfix.
type ServiceNotAsClassMatch struct {
	SourceFile     *ast.SourceFile
	Location       core.TextRange // Error range for the call expression
	SuggestedUsage string         // The full suggested class declaration string for the diagnostic message
	CallExprNode   *ast.Node      // The call expression node (ServiceMap.Service<...>(...))
	VariableName   string         // The variable/class name
	TypeArgsText   string         // Text of original type arguments (e.g. "ConfigService")
	ArgsText       string         // Text of original call arguments (e.g. `"Config"`)
	TargetNode     *ast.Node      // The node to replace (variable statement or declaration list)
	ModifierNodes  *ast.ModifierList // Modifiers from the variable statement (e.g. export)
}

// AnalyzeServiceNotAsClass finds all const variable declarations using ServiceMap.Service
// that should be class declarations instead. V4-only rule.
func AnalyzeServiceNotAsClass(c *checker.Checker, sf *ast.SourceFile) []ServiceNotAsClassMatch {
	if typeparser.SupportedEffectVersion(c) != typeparser.EffectMajorV4 {
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
			if m := checkServiceNotAsClass(c, sf, node); m != nil {
				matches = append(matches, *m)
			}
		}

		node.ForEachChild(pushChild)
	}

	return matches
}

func checkServiceNotAsClass(c *checker.Checker, sf *ast.SourceFile, node *ast.Node) *ServiceNotAsClassMatch {
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

	// Check the call expression references ServiceMap.Service
	if !typeparser.IsNodeReferenceToServiceMapModuleApi(c, callExpr.Expression, "Service") {
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

	// Build suggested usage string: class VariableName extends ServiceMap.Service<VariableName, TypeArgs>()(Args) {}
	var suggestedUsage string
	if argsText != "" {
		suggestedUsage = fmt.Sprintf("class %s extends ServiceMap.Service<%s, %s>()(%s) {}", variableName, variableName, typeArgsText, argsText)
	} else {
		suggestedUsage = fmt.Sprintf("class %s extends ServiceMap.Service<%s, %s>() {}", variableName, variableName, typeArgsText)
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
