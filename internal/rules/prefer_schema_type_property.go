package rules

import (
	"slices"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var PreferSchemaTypeProperty = rule.Rule{
	Name:            "preferSchemaTypeProperty",
	Group:           "style",
	Description:     "Disallows Schema.Schema.Type<typeof X> and Schema.Codec.Encoded<typeof X>-style helpers in favor of typeof X.Type and typeof X.Encoded",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Do_not_use_0_typeof_X_Use_typeof_X_1_instead_effect_preferSchemaTypeProperty.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzePreferSchemaTypeProperty(ctx.TypeParser, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(match.SourceFile, match.Location, tsdiag.Do_not_use_0_typeof_X_Use_typeof_X_1_instead_effect_preferSchemaTypeProperty, nil, match.HelperText, match.PropertyName)
		}
		return diags
	},
}

type PreferSchemaTypePropertyMatch struct {
	SourceFile     *ast.SourceFile
	Location       core.TextRange
	TypeReference  *ast.Node
	SchemaName     *ast.Node
	SchemaNameText string
	HelperText     string
	PropertyName   string
}

// Each helper extracts the schema property of the same name; Encoded lives on
// Schema in Effect v3 and on Codec in Effect v4.
var schemaPropertyHelperNamespaces = map[string][]string{
	"Type":             {"Schema"},
	"Encoded":          {"Schema", "Codec"},
	"Context":          {"Schema"},
	"DecodingServices": {"Codec"},
	"EncodingServices": {"Codec"},
}

func AnalyzePreferSchemaTypeProperty(tp *typeparser.TypeParser, sf *ast.SourceFile) []PreferSchemaTypePropertyMatch {
	var matches []PreferSchemaTypePropertyMatch
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindTypeReference {
			if match := analyzePreferSchemaTypePropertyNode(tp, sf, node); match != nil {
				matches = append(matches, *match)
				return false
			}
		}
		//ets:ast-walk finds Schema.Type<typeof X>-style type references, which are type nodes
		node.ForEachChild(walk)
		return false
	}
	walk(sf.AsNode())
	return matches
}

func analyzePreferSchemaTypePropertyNode(tp *typeparser.TypeParser, sf *ast.SourceFile, node *ast.Node) *PreferSchemaTypePropertyMatch {
	reference := node.AsTypeReferenceNode()
	if reference == nil || reference.TypeName == nil || reference.TypeArguments == nil || len(reference.TypeArguments.Nodes) != 1 {
		return nil
	}

	typeName := reference.TypeName.AsNode()
	if typeName == nil || typeName.Kind != ast.KindQualifiedName {
		return nil
	}
	outerName := typeName.AsQualifiedName()
	if outerName.Right == nil || outerName.Left == nil {
		return nil
	}
	propertyName := outerName.Right.Text()
	namespaces, ok := schemaPropertyHelperNamespaces[propertyName]
	if !ok {
		return nil
	}
	namespaceName := outerName.Left.AsNode()
	if namespaceName != nil && namespaceName.Kind == ast.KindQualifiedName {
		namespaceName = namespaceName.AsQualifiedName().Right
	}
	if namespaceName == nil || namespaceName.Kind != ast.KindIdentifier {
		return nil
	}
	typeArgument := reference.TypeArguments.Nodes[0]
	if typeArgument == nil || typeArgument.Kind != ast.KindTypeQuery {
		return nil
	}
	query := typeArgument.AsTypeQueryNode()
	if query.ExprName == nil || query.TypeArguments != nil && len(query.TypeArguments.Nodes) > 0 {
		return nil
	}

	if !slices.ContainsFunc(namespaces, func(namespace string) bool {
		return tp.IsNodeReferenceToEffectSchemaModuleApi(namespaceName, namespace)
	}) {
		return nil
	}

	return &PreferSchemaTypePropertyMatch{
		SourceFile:     sf,
		Location:       scanner.GetErrorRangeForNode(sf, node),
		TypeReference:  node,
		SchemaName:     query.ExprName.AsNode(),
		SchemaNameText: scanner.GetTextOfNode(query.ExprName.AsNode()),
		HelperText:     scanner.GetTextOfNode(typeName),
		PropertyName:   propertyName,
	}
}
