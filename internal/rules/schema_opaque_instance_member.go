package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
)

var SchemaOpaqueInstanceMember = rule.Rule{
	Name:            "schemaOpaqueInstanceMember",
	Group:           "correctness",
	Description:     "Disallows instance members in classes extending Schema.Opaque",
	DefaultSeverity: etscore.SeverityError,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.Classes_extending_Schema_Opaque_must_not_declare_instance_members_effect_schemaOpaqueInstanceMember.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diagnostics []*ast.Diagnostic
		var walk ast.Visitor
		walk = func(node *ast.Node) bool {
			if (node.Kind == ast.KindClassDeclaration || node.Kind == ast.KindClassExpression) && ctx.TypeParser.ExtendsSchemaOpaque(node) != nil {
				for _, member := range node.Members() {
					if isSchemaOpaqueInstanceMember(member) {
						diagnostics = append(diagnostics, ctx.NewDiagnostic(
							ctx.SourceFile,
							ctx.GetErrorRange(member),
							tsdiag.Classes_extending_Schema_Opaque_must_not_declare_instance_members_effect_schemaOpaqueInstanceMember,
							nil,
						))
					}
				}
			}
			node.ForEachChild(walk)
			return false
		}
		walk(ctx.SourceFile.AsNode())
		return diagnostics
	},
}

func isSchemaOpaqueInstanceMember(node *ast.Node) bool {
	if node == nil || ast.HasSyntacticModifier(node, ast.ModifierFlagsStatic) {
		return false
	}
	switch node.Kind {
	case ast.KindPropertyDeclaration, ast.KindMethodDeclaration, ast.KindGetAccessor, ast.KindSetAccessor, ast.KindConstructor:
		return true
	default:
		return false
	}
}
