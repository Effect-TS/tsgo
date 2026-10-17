package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var ErrorPayloadShadowsName = rule.Rule{
	Name:            "errorPayloadShadowsName",
	Group:           "correctness",
	Description:     "Detects error payload fields named `name` that shadow the error's own name",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.The_payload_field_name_shadows_this_error_s_own_name_so_logs_and_String_error_show_the_field_value_instead_of_the_error_type_Consider_renaming_it_effect_errorPayloadShadowsName.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeErrorPayloadShadowsName(ctx.TypeParser, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(match.SourceFile, match.Location, tsdiag.The_payload_field_name_shadows_this_error_s_own_name_so_logs_and_String_error_show_the_field_value_instead_of_the_error_type_Consider_renaming_it_effect_errorPayloadShadowsName, nil)
		}
		return diags
	},
}

type ErrorPayloadShadowsNameMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
}

func AnalyzeErrorPayloadShadowsName(tp *typeparser.TypeParser, sf *ast.SourceFile) []ErrorPayloadShadowsNameMatch {
	var matches []ErrorPayloadShadowsNameMatch
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node.Kind == ast.KindClassDeclaration || node.Kind == ast.KindClassExpression {
			fields, errorName := errorPayloadFields(tp, node)
			var members []*ast.Node
			if fields != nil {
				switch fields.Kind {
				case ast.KindTypeLiteral:
					members = fields.Members()
				case ast.KindObjectLiteralExpression:
					members = fields.Properties()
				}
			}
			for _, member := range members {
				switch member.Kind {
				case ast.KindPropertySignature, ast.KindPropertyAssignment, ast.KindShorthandPropertyAssignment:
					name := member.Name()
					if name == nil || ast.GetTextOfPropertyName(name) != "name" || errorPayloadNameMatchesErrorName(tp, member, errorName) {
						continue
					}
					matches = append(matches, ErrorPayloadShadowsNameMatch{
						SourceFile: sf,
						Location:   scanner.GetErrorRangeForNode(sf, name),
					})
				}
			}
		}
		node.ForEachChild(walk)
		return false
	}
	walk(sf.AsNode())
	return matches
}

func errorPayloadFields(tp *typeparser.TypeParser, classNode *ast.Node) (fields, errorName *ast.Node) {
	if result := tp.ExtendsDataTaggedError(classNode); result != nil {
		return result.FieldsTypeNode(), result.KeyStringLiteral()
	}
	if result := tp.ExtendsDataError(classNode); result != nil {
		return result.FieldsTypeNode(), nil
	}
	if result := tp.ExtendsSchemaTaggedError(classNode); result != nil {
		if tp.SupportedEffectVersion() == typeparser.EffectMajorV4 && result.KeyStringLiteral() != nil {
			return result.FieldsNode(), result.KeyStringLiteral()
		}
		return result.FieldsNode(), result.TagStringLiteral()
	}
	if result := tp.ExtendsSchemaError(classNode); result != nil {
		return result.FieldsNode(), result.KeyStringLiteral()
	}
	return nil, nil
}

func errorPayloadNameMatchesErrorName(tp *typeparser.TypeParser, member, errorName *ast.Node) bool {
	if errorName == nil || !ast.IsStringLiteral(errorName) {
		return false
	}
	var literal *ast.Node
	switch member.Kind {
	case ast.KindPropertySignature:
		typeNode := member.AsPropertySignatureDeclaration().Type
		if typeNode != nil && typeNode.Kind == ast.KindLiteralType {
			literal = typeNode.AsLiteralTypeNode().Literal
		}
	case ast.KindPropertyAssignment:
		initializer := member.AsPropertyAssignment().Initializer
		if initializer != nil && ast.IsCallExpression(initializer) {
			call := initializer.AsCallExpression()
			if call.Arguments != nil && len(call.Arguments.Nodes) == 1 &&
				(tp.IsNodeReferenceToEffectSchemaModuleApi(call.Expression, "Literal") ||
					tp.IsNodeReferenceToEffectSchemaModuleApi(call.Expression, "tag") ||
					tp.IsNodeReferenceToEffectSchemaModuleApi(call.Expression, "tagDefaultOmit")) {
				literal = call.Arguments.Nodes[0]
			}
		}
	}
	return literal != nil && ast.IsStringLiteral(literal) && literal.Text() == errorName.Text()
}
