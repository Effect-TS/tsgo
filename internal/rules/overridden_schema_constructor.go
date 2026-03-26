package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var OverriddenSchemaConstructor = rule.Rule{
	Name:            "overriddenSchemaConstructor",
	Group:           "correctness",
	Description:     "Prevents overriding constructors in Schema classes which breaks decoding behavior",
	DefaultSeverity: etscore.SeverityError,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Classes_extending_Schema_must_not_override_the_constructor_this_is_because_it_silently_breaks_the_schema_decoding_behaviour_If_that_s_needed_we_recommend_instead_to_use_a_static_new_method_that_constructs_the_instance_effect_overriddenSchemaConstructor.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeOverriddenSchemaConstructor(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.Classes_extending_Schema_must_not_override_the_constructor_this_is_because_it_silently_breaks_the_schema_decoding_behaviour_If_that_s_needed_we_recommend_instead_to_use_a_static_new_method_that_constructs_the_instance_effect_overriddenSchemaConstructor, nil)
		}
		return diags
	},
}

// OverriddenSchemaConstructorMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fixes for the overriddenSchemaConstructor pattern.
type OverriddenSchemaConstructorMatch struct {
	SourceFile      *ast.SourceFile
	Location        core.TextRange // The error range for the constructor node
	ConstructorNode *ast.Node      // The constructor declaration AST node
	HasBody         bool           // Whether the constructor has a body (the static fix is only available when true)
}

// AnalyzeOverriddenSchemaConstructor finds all class declarations extending Schema
// that have an overridden constructor which is not an allowed passthrough pattern.
func AnalyzeOverriddenSchemaConstructor(c *checker.Checker, sf *ast.SourceFile) []OverriddenSchemaConstructorMatch {
	var matches []OverriddenSchemaConstructorMatch

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
			if m := checkOverriddenSchemaConstructor(c, sf, node); m != nil {
				matches = append(matches, *m)
			}
		}

		node.ForEachChild(pushChild)
	}

	return matches
}

func checkOverriddenSchemaConstructor(c *checker.Checker, sf *ast.SourceFile, node *ast.Node) *OverriddenSchemaConstructorMatch {
	extendsElements := ast.GetExtendsHeritageClauseElements(node)
	if len(extendsElements) == 0 {
		return nil
	}

	extendsSchema := false
	for _, elem := range extendsElements {
		if elem.Kind != ast.KindExpressionWithTypeArguments {
			continue
		}
		expr := elem.AsExpressionWithTypeArguments().Expression
		t := typeparser.GetTypeAtLocation(c, expr)
		if t != nil && typeparser.IsSchemaType(c, t, expr) {
			extendsSchema = true
			break
		}
	}

	if !extendsSchema {
		return nil
	}

	if node.Kind != ast.KindClassDeclaration {
		return nil
	}
	classDecl := node.AsClassDeclaration()
	if classDecl.Members == nil {
		return nil
	}

	for _, member := range classDecl.Members.Nodes {
		if member.Kind == ast.KindConstructor {
			if isAllowedConstructor(member) {
				continue
			}
			ctor := member.AsConstructorDeclaration()
			hasBody := ctor.Body != nil && ctor.Body.Kind == ast.KindBlock
			return &OverriddenSchemaConstructorMatch{
				SourceFile:      sf,
				Location:        scanner.GetErrorRangeForNode(sf, member),
				ConstructorNode: member,
				HasBody:         hasBody,
			}
		}
	}

	return nil
}

// isAllowedConstructor checks if a constructor is a passthrough that simply forwards
// exactly 2 parameters to super(). This pattern is used internally by Schema and is allowed.
func isAllowedConstructor(ctorNode *ast.Node) bool {
	ctor := ctorNode.AsConstructorDeclaration()
	if ctor.Body == nil || ctor.Body.Kind != ast.KindBlock {
		return false
	}
	block := ctor.Body.AsBlock()
	if block.Statements == nil || len(block.Statements.Nodes) != 1 {
		return false
	}

	stmt := block.Statements.Nodes[0]
	if stmt.Kind != ast.KindExpressionStatement {
		return false
	}

	expr := stmt.AsExpressionStatement().Expression
	if expr.Kind != ast.KindCallExpression {
		return false
	}

	call := expr.AsCallExpression()
	if call.Expression.Kind != ast.KindSuperKeyword {
		return false
	}

	// Constructor must have exactly 2 parameters
	if ctor.Parameters == nil || len(ctor.Parameters.Nodes) != 2 {
		return false
	}

	// Collect parameter names (must all be simple identifiers)
	expectedNames := make([]string, 0, 2)
	for _, param := range ctor.Parameters.Nodes {
		name := param.Name()
		if name == nil || name.Kind != ast.KindIdentifier {
			return false
		}
		expectedNames = append(expectedNames, scanner.GetTextOfNode(name))
	}

	// super() call must have exactly 2 arguments matching parameter names in order
	if call.Arguments == nil || len(call.Arguments.Nodes) != 2 {
		return false
	}

	for i, arg := range call.Arguments.Nodes {
		if arg.Kind != ast.KindIdentifier {
			return false
		}
		if scanner.GetTextOfNode(arg) != expectedNames[i] {
			return false
		}
	}

	return true
}
