package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/readonly"
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
	// Flow starts with the subject {} and the Schema.Struct step.
	Flow *typeparser.PipingFlow
	// InToolParameters is set when the flow is the parameters of Tool.make or Tool.dynamic.
	InToolParameters bool
	// ToolNamespace is the Tool module receiver of the tool constructor
	// (Tool in Tool.make), or nil when the constructor is not accessed on it.
	ToolNamespace *ast.Node
}

// AnalyzeEmptySchemaStruct finds every flow that applies Schema.Struct to an
// empty object literal and returns match structs for each.
func AnalyzeEmptySchemaStruct(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []EmptySchemaStructMatch {
	flows := tp.PipingFlows(sf, false)
	toolParameters := emptySchemaStructToolParameters(tp, flows)

	var matches []EmptySchemaStructMatch
	for _, flow := range flows.All() {
		if !flow.MatchesPrefix(
			func(subject typeparser.PipingFlowSubject) bool {
				// Spreads and computed keys are properties too
				node := subject.Node()
				if node == nil || node.Kind != ast.KindObjectLiteralExpression {
					return false
				}
				properties := node.AsObjectLiteralExpression().Properties
				return properties == nil || len(properties.Nodes) == 0
			},
			func(t typeparser.PipingFlowTransformation) bool {
				return t.Args().Len() == 0 && tp.IsNodeReferenceToEffectSchemaModuleApi(t.Callee(), "Struct")
			},
		) {
			continue
		}

		// The flow node is the Schema.Struct({}) expression itself only when no step follows it
		node := flow.Node()
		structOnly := flow.Transformations().Len() == 1

		// Schema.StructWithRest(Schema.Struct({}), records) adds index signatures to the empty struct
		if structOnly && isStructWithRestFields(tp, node) {
			continue
		}

		// A call step is reported on its call; other spellings on the callee
		step := flow.Transformations().At(0)
		location := step.Callee()
		if step.Kind() == typeparser.TransformationKindCall {
			location = step.Callee().Parent
		}

		var toolNamespace *ast.Node
		inToolParameters := false
		if structOnly {
			toolNamespace, inToolParameters = toolParameters[node]
		}
		matches = append(matches, EmptySchemaStructMatch{
			SourceFile:       sf,
			Location:         scanner.GetErrorRangeForNode(sf, location),
			Flow:             flow,
			InToolParameters: inToolParameters,
			ToolNamespace:    toolNamespace,
		})
	}
	return matches
}

// isStructWithRestFields reports whether node is the fields argument of
// Schema.StructWithRest, a call that never becomes a flow step.
func isStructWithRestFields(tp *typeparser.TypeParser, node *ast.Node) bool {
	parent := node.Parent
	if parent == nil || parent.Kind != ast.KindCallExpression {
		return false
	}
	call := parent.AsCallExpression()
	return call.Arguments != nil && len(call.Arguments.Nodes) > 0 && call.Arguments.Nodes[0] == node &&
		tp.IsNodeReferenceToEffectSchemaModuleApi(call.Expression, "StructWithRest")
}

// emptySchemaStructToolParameters maps the parameters value of each Tool.make
// and Tool.dynamic step to the Tool module receiver of its constructor, or nil
// when the constructor is not accessed on it.
func emptySchemaStructToolParameters(tp *typeparser.TypeParser, flows readonly.Slice[*typeparser.PipingFlow]) map[*ast.Node]*ast.Node {
	parameters := map[*ast.Node]*ast.Node{}
	for _, flow := range flows.All() {
		for _, t := range flow.Transformations().All() {
			// Tool.make(name, options) applies the step to the name with the options as its argument
			if t.Args().Len() != 1 ||
				(!tp.IsNodeReferenceToEffectAiToolModuleApi(t.Callee(), "make") && !tp.IsNodeReferenceToEffectAiToolModuleApi(t.Callee(), "dynamic")) {
				continue
			}
			options := t.Args().At(0)
			if options.Kind != ast.KindObjectLiteralExpression {
				continue
			}
			var toolNamespace *ast.Node
			if ast.IsPropertyAccessExpression(t.Callee()) {
				if receiver := t.Callee().AsPropertyAccessExpression().Expression; tp.IsExpressionEffectAiToolModule(receiver) {
					toolNamespace = receiver
				}
			}
			properties := options.AsObjectLiteralExpression().Properties
			if properties == nil {
				continue
			}
			for _, property := range properties.Nodes {
				if property.Kind == ast.KindPropertyAssignment && ast.GetTextOfPropertyName(property.Name()) == "parameters" {
					parameters[ast.SkipParentheses(property.Initializer())] = toolNamespace
				}
			}
		}
	}
	return parameters
}
