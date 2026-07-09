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

// NewSchemaClass suggests using the Schema make API instead of direct
// construction with new for Schema classes.
var NewSchemaClass = rule.Rule{
	Name:            "newSchemaClass",
	Group:           "style",
	Description:     "Suggests using Schema make instead of new for Schema classes",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.This_Schema_class_is_constructed_with_new_0_make_can_be_used_to_construct_the_Schema_class_instance_effect_newSchemaClass.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeNewSchemaClass(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.This_Schema_class_is_constructed_with_new_0_make_can_be_used_to_construct_the_Schema_class_instance_effect_newSchemaClass,
				nil,
				match.ClassText,
			)
		}
		return diags
	},
}

type NewSchemaClassMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	NewNode    *ast.Node
	ClassExpr  *ast.Node
	ClassText  string
	Arguments  []*ast.Node
}

func AnalyzeNewSchemaClass(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []NewSchemaClassMatch {
	if tp == nil || tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	var matches []NewSchemaClassMatch

	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}

		if node.Kind == ast.KindNewExpression {
			newExpr := node.AsNewExpression()
			if newExpr != nil && newExpr.Expression != nil && isV4SchemaClassExpression(tp, c, newExpr.Expression) {
				var args []*ast.Node
				if newExpr.Arguments != nil {
					args = newExpr.Arguments.Nodes
				}
				matches = append(matches, NewSchemaClassMatch{
					SourceFile: sf,
					Location:   scanner.GetErrorRangeForNode(sf, newExpr.Expression),
					NewNode:    node,
					ClassExpr:  newExpr.Expression,
					ClassText:  scanner.GetSourceTextOfNodeFromSourceFile(sf, newExpr.Expression, false),
					Arguments:  args,
				})
			}
		}

		node.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())
	return matches
}

func isV4SchemaClassExpression(tp *typeparser.TypeParser, c *checker.Checker, expr *ast.Node) bool {
	if tp == nil || c == nil || expr == nil {
		return false
	}

	t := tp.GetTypeAtLocation(expr)
	if t == nil || !tp.IsSchemaType(t, expr) {
		return false
	}

	if len(c.GetSignaturesOfType(t, checker.SignatureKindConstruct)) == 0 {
		return false
	}

	return tp.GetTypeOfPropertyByName(t, "make") != nil
}
