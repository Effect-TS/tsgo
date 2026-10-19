package rules

import (
	"slices"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

var SchemaClassObjectLiteral = rule.Rule{
	Name:            "schemaClassObjectLiteral",
	Group:           "correctness",
	Description:     "Warns when object literals are checked as Schema class instances without the class prototype",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_object_literal_is_checked_as_an_instance_of_Schema_class_0_but_has_no_class_prototype_instanceof_checks_methods_and_Schema_encoding_can_fail_Use_0_make_or_new_0_instead_effect_schemaClassObjectLiteral.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		classes := make(map[*ast.Symbol]bool)
		var isSchemaClassInstance func(*checker.Type) bool
		isSchemaClassInstance = func(t *checker.Type) bool {
			if t == nil || t.ObjectFlags()&(checker.ObjectFlagsClass|checker.ObjectFlagsReference) == 0 {
				return false
			}
			symbol := t.Symbol()
			if symbol == nil || ast.TSGetSymbolFlags(symbol)&ast.SymbolFlagsClass == 0 {
				return false
			}
			if result, ok := classes[symbol]; ok {
				return result
			}
			classes[symbol] = false
			for _, declaration := range ast.TSGetSymbolDeclarations(symbol) {
				if declaration.Kind == ast.KindClassDeclaration || declaration.Kind == ast.KindClassExpression {
					if self, _ := schemaClassInstanceSelfAndBrand(ctx.TypeParser, declaration); self != nil {
						classes[symbol] = true
						return true
					}
				}
			}
			classes[symbol] = slices.ContainsFunc(ctx.Checker.GetBaseTypes(ctx.Checker.GetDeclaredTypeOfSymbol(symbol)), isSchemaClassInstance)
			return classes[symbol]
		}

		var diagnostics []*ast.Diagnostic
		var walk ast.Visitor
		walk = func(node *ast.Node) bool {
			if node.Kind == ast.KindObjectLiteralExpression {
				contextualType := ctx.TypeParser.CachedObjectLiteralContextualType(node)
				var className string
				for _, member := range ctx.TypeParser.UnrollUnionMembers(contextualType) {
					if member.Flags()&(checker.TypeFlagsNull|checker.TypeFlagsUndefined) != 0 {
						continue
					}
					if !isSchemaClassInstance(member) {
						className = ""
						break
					}
					if className == "" {
						className = ast.TSGetSymbolName(member.Symbol())
					}
				}
				if className != "" {
					diagnostics = append(diagnostics, ctx.NewDiagnostic(
						ctx.SourceFile,
						ctx.GetErrorRange(node),
						tsdiag.This_object_literal_is_checked_as_an_instance_of_Schema_class_0_but_has_no_class_prototype_instanceof_checks_methods_and_Schema_encoding_can_fail_Use_0_make_or_new_0_instead_effect_schemaClassObjectLiteral,
						nil, className,
					))
				}
			}
			node.ForEachChild(walk)
			return false
		}
		walk(ctx.SourceFile.AsNode())
		return diagnostics
	},
}
