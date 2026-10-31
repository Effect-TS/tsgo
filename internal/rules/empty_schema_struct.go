package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// EmptySchemaStruct detects Schema.Struct({}) calls. The schema models the
// type {}, so decoding accepts any non-null value without validating it.
var EmptySchemaStruct = rule.Rule{
	Name:            "emptySchemaStruct",
	Group:           "correctness",
	Description:     "Detects Schema.Struct({}), which accepts any non-null value without validation",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Schema_Struct_models_the_type_so_any_non_null_value_passes_decoding_unvalidated_Use_an_object_keyword_schema_Schema_ObjectKeyword_in_v4_Schema_Object_in_v3_for_any_object_a_record_schema_for_open_key_Slashvalue_maps_or_add_the_intended_fields_effect_emptySchemaStruct.Code(),
		tsdiag.Schema_Struct_as_tool_parameters_breaks_structured_output_schema_generation_for_some_providers_Use_Tool_EmptyParams_instead_effect_emptySchemaStruct.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeEmptySchemaStruct(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			message := tsdiag.Schema_Struct_models_the_type_so_any_non_null_value_passes_decoding_unvalidated_Use_an_object_keyword_schema_Schema_ObjectKeyword_in_v4_Schema_Object_in_v3_for_any_object_a_record_schema_for_open_key_Slashvalue_maps_or_add_the_intended_fields_effect_emptySchemaStruct
			if m.InToolParameters {
				message = tsdiag.Schema_Struct_as_tool_parameters_breaks_structured_output_schema_generation_for_some_providers_Use_Tool_EmptyParams_instead_effect_emptySchemaStruct
			}
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, message, nil)
		}
		return diags
	},
}

// EmptySchemaStructMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the emptySchemaStruct pattern.
type EmptySchemaStructMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange // Pre-computed error range for the diagnostic
	Call       *ast.Node      // The Schema.Struct({}) call expression
	// InToolParameters is set when Call is the parameters of Tool.make or Tool.dynamic.
	InToolParameters bool
	// ToolNamespace is the Tool module receiver of the tool constructor
	// (Tool in Tool.make), or nil when the constructor is not accessed on it.
	ToolNamespace *ast.Node
}

// AnalyzeEmptySchemaStruct finds all Schema.Struct({}) call expressions and
// returns match structs for each.
func AnalyzeEmptySchemaStruct(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []EmptySchemaStructMatch {
	var matches []EmptySchemaStructMatch

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

		if node.Kind == ast.KindCallExpression {
			if m, ok := analyzeEmptySchemaStructNode(tp, sf, node); ok {
				matches = append(matches, m)
			}
		}

		// Enqueue children
		node.ForEachChild(pushChild)
	}

	return matches
}

// analyzeEmptySchemaStructNode checks a call expression for Schema.Struct({}).
func analyzeEmptySchemaStructNode(tp *typeparser.TypeParser, sf *ast.SourceFile, node *ast.Node) (EmptySchemaStructMatch, bool) {
	call := node.AsCallExpression()

	// A single argument: v3 index-signature records follow the fields as extra arguments
	if call.Arguments == nil || len(call.Arguments.Nodes) != 1 {
		return EmptySchemaStructMatch{}, false
	}

	// The argument must be a literal {}; spreads and computed keys are properties too
	arg := ast.SkipParentheses(call.Arguments.Nodes[0])
	if arg.Kind != ast.KindObjectLiteralExpression {
		return EmptySchemaStructMatch{}, false
	}
	if properties := arg.AsObjectLiteralExpression().Properties; properties != nil && len(properties.Nodes) > 0 {
		return EmptySchemaStructMatch{}, false
	}

	if !tp.IsNodeReferenceToEffectSchemaModuleApi(call.Expression, "Struct") {
		return EmptySchemaStructMatch{}, false
	}

	// Schema.StructWithRest(Schema.Struct({}), records) adds index signatures to the empty struct
	if parent := ast.WalkUpParenthesizedExpressions(node.Parent); parent != nil && parent.Kind == ast.KindCallExpression {
		parentCall := parent.AsCallExpression()
		if parentCall.Arguments != nil && len(parentCall.Arguments.Nodes) > 0 && ast.SkipParentheses(parentCall.Arguments.Nodes[0]) == node &&
			tp.IsNodeReferenceToEffectSchemaModuleApi(parentCall.Expression, "StructWithRest") {
			return EmptySchemaStructMatch{}, false
		}
	}

	inToolParameters, toolNamespace := emptySchemaStructToolContext(tp, node)
	return EmptySchemaStructMatch{
		SourceFile:       sf,
		Location:         scanner.GetErrorRangeForNode(sf, node),
		Call:             node,
		InToolParameters: inToolParameters,
		ToolNamespace:    toolNamespace,
	}, true
}

// emptySchemaStructToolContext reports whether call is the parameters property
// of the options passed to Tool.make or Tool.dynamic, and returns the Tool
// module receiver of the constructor when there is one.
func emptySchemaStructToolContext(tp *typeparser.TypeParser, call *ast.Node) (bool, *ast.Node) {
	// A computed name has a ComputedPropertyName parent, so call is the initializer here
	property := ast.WalkUpParenthesizedExpressions(call.Parent)
	if property == nil || property.Kind != ast.KindPropertyAssignment ||
		ast.GetTextOfPropertyName(property.Name()) != "parameters" {
		return false, nil
	}

	options := property.Parent
	if options == nil || options.Kind != ast.KindObjectLiteralExpression {
		return false, nil
	}
	toolCall := ast.WalkUpParenthesizedExpressions(options.Parent)
	if toolCall == nil || toolCall.Kind != ast.KindCallExpression {
		return false, nil
	}
	// Tool.make and Tool.dynamic take (name, options)
	arguments := toolCall.AsCallExpression().Arguments
	if arguments == nil || len(arguments.Nodes) < 2 || ast.SkipParentheses(arguments.Nodes[1]) != options {
		return false, nil
	}
	callee := toolCall.AsCallExpression().Expression
	if !tp.IsNodeReferenceToEffectAiToolModuleApi(callee, "make") && !tp.IsNodeReferenceToEffectAiToolModuleApi(callee, "dynamic") {
		return false, nil
	}

	if ast.IsPropertyAccessExpression(callee) {
		if receiver := callee.AsPropertyAccessExpression().Expression; tp.IsExpressionEffectAiToolModule(receiver) {
			return true, receiver
		}
	}
	return true, nil
}
