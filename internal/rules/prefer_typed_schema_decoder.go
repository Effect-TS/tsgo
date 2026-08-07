package rules

import (
	"sort"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var typedSchemaDecoders = map[string]string{
	"decodeUnknownEffect":  "decodeEffect",
	"decodeUnknownSync":    "decodeSync",
	"decodeUnknownExit":    "decodeExit",
	"decodeUnknownOption":  "decodeOption",
	"decodeUnknownResult":  "decodeResult",
	"decodeUnknownPromise": "decodePromise",
}

// PreferTypedSchemaDecoder suggests typed Schema decoders when the input is
// statically assignable to the schema's Encoded type.
var PreferTypedSchemaDecoder = rule.Rule{
	Name:            "preferTypedSchemaDecoder",
	Group:           "style",
	Description:     "Suggests typed Schema decoders when the input is assignable to the schema's Encoded type",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.This_input_is_already_assignable_to_the_schema_s_Encoded_type_Use_0_to_preserve_compile_time_type_checking_instead_of_discarding_the_input_type_through_1_effect_preferTypedSchemaDecoder.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzePreferTypedSchemaDecoder(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.This_input_is_already_assignable_to_the_schema_s_Encoded_type_Use_0_to_preserve_compile_time_type_checking_instead_of_discarding_the_input_type_through_1_effect_preferTypedSchemaDecoder,
				nil,
				match.TypedName,
				match.UnknownName,
			)
		}
		return diags
	},
}

type PreferTypedSchemaDecoderMatch struct {
	SourceFile  *ast.SourceFile
	Location    core.TextRange
	DecoderName *ast.Node
	UnknownName string
	TypedName   string
}

// AnalyzePreferTypedSchemaDecoder finds unknown-input decoder applications whose
// input is statically accepted by the schema's Encoded type.
func AnalyzePreferTypedSchemaDecoder(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []PreferTypedSchemaDecoderMatch {
	if tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	var matches []PreferTypedSchemaDecoderMatch
	seen := make(map[*ast.Node]bool)

	for _, flow := range tp.PipingFlows(sf, false) {
		inputNode := flow.Subject.Node
		inputType := flow.Subject.OutType
		for _, transformation := range flow.Transformations {
			callee, schema := schemaDecoderTransformation(transformation)
			if callee != nil && schema != nil && !seen[transformation.Node] {
				if match := analyzeTypedSchemaDecoderApplication(tp, c, sf, callee, schema, inputNode, inputType); match != nil {
					matches = append(matches, *match)
					seen[transformation.Node] = true
				}
			}
			inputNode = nil
			inputType = transformation.OutType
		}
	}

	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindCallExpression && !seen[node] {
			call := node.AsCallExpression()
			if call.Arguments != nil && len(call.Arguments.Nodes) > 0 && call.Expression != nil && call.Expression.Kind == ast.KindCallExpression {
				factory := call.Expression.AsCallExpression()
				if factory.Arguments != nil && len(factory.Arguments.Nodes) > 0 {
					if match := analyzeTypedSchemaDecoderApplication(tp, c, sf, factory.Expression, factory.Arguments.Nodes[0], call.Arguments.Nodes[0], nil); match != nil {
						matches = append(matches, *match)
						seen[node] = true
					}
				}
			}
		}
		node.ForEachChild(walk)
		return false
	}
	walk(sf.AsNode())
	sort.Slice(matches, func(i, j int) bool {
		return matches[i].Location.Pos() < matches[j].Location.Pos()
	})

	return matches
}

func schemaDecoderTransformation(transformation typeparser.PipingFlowTransformation) (callee *ast.Node, schema *ast.Node) {
	callee = transformation.Callee
	args := transformation.Args
	if len(args) == 0 && callee != nil && callee.Kind == ast.KindCallExpression {
		call := callee.AsCallExpression()
		callee = call.Expression
		if call.Arguments != nil {
			args = call.Arguments.Nodes
		}
	}
	if callee == nil || len(args) == 0 {
		return nil, nil
	}
	return callee, args[0]
}

func analyzeTypedSchemaDecoderApplication(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, callee, schema, inputNode *ast.Node, inputType *checker.Type) *PreferTypedSchemaDecoderMatch {
	unknownName, typedName := matchUnknownSchemaDecoder(tp, callee)
	if unknownName == "" {
		return nil
	}

	schemaType := tp.EffectSchemaTypes(tp.GetTypeAtLocation(schema), schema)
	if schemaType == nil || schemaType.E == nil {
		return nil
	}
	if inputType == nil && inputNode != nil {
		inputType = tp.GetTypeAtLocation(inputNode)
	}
	if inputType == nil || inputType.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown) != 0 || containsUnresolvedTypeVariable(c, inputType, make(map[*checker.Type]bool)) {
		return nil
	}

	assignableType := inputType
	if inputNode != nil {
		if literal := ast.SkipParentheses(inputNode); literal != nil && (literal.Kind == ast.KindObjectLiteralExpression || literal.Kind == ast.KindArrayLiteralExpression) {
			assignableType = checker.Checker_checkExpressionWithContextualType(c, literal, schemaType.E, nil, checker.CheckModeTypeOnly)
		}
	}
	if assignableType == nil || !checker.Checker_isTypeAssignableTo(c, assignableType, schemaType.E) {
		return nil
	}

	nameNode := schemaDecoderNameNode(callee)
	if nameNode == nil {
		return nil
	}
	return &PreferTypedSchemaDecoderMatch{
		SourceFile:  sf,
		Location:    scanner.GetErrorRangeForNode(sf, nameNode),
		DecoderName: nameNode,
		UnknownName: unknownName,
		TypedName:   typedName,
	}
}

func containsUnresolvedTypeVariable(c *checker.Checker, t *checker.Type, seen map[*checker.Type]bool) bool {
	if t == nil || seen[t] {
		return false
	}
	seen[t] = true
	if t.Flags()&checker.TypeFlagsTypeVariable != 0 {
		return true
	}
	if t.Flags()&checker.TypeFlagsUnionOrIntersection != 0 {
		for _, part := range t.Types() {
			if containsUnresolvedTypeVariable(c, part, seen) {
				return true
			}
		}
	}
	if t.ObjectFlags()&checker.ObjectFlagsReference != 0 {
		for _, arg := range checker.Checker_getTypeArguments(c, t) {
			if containsUnresolvedTypeVariable(c, arg, seen) {
				return true
			}
		}
	}
	for _, property := range c.GetPropertiesOfType(t) {
		if containsUnresolvedTypeVariable(c, c.GetTypeOfSymbol(property), seen) {
			return true
		}
	}
	for _, indexInfo := range checker.Checker_getIndexInfosOfType(c, t) {
		if containsUnresolvedTypeVariable(c, checker.IndexInfo_valueType(indexInfo), seen) {
			return true
		}
	}
	return false
}

func matchUnknownSchemaDecoder(tp *typeparser.TypeParser, callee *ast.Node) (string, string) {
	for unknownName, typedName := range typedSchemaDecoders {
		if tp.IsNodeReferenceToEffectSchemaModuleApi(callee, unknownName) ||
			tp.IsNodeReferenceToEffectSchemaParserModuleApi(callee, unknownName) {
			return unknownName, typedName
		}
	}
	return "", ""
}

func schemaDecoderNameNode(callee *ast.Node) *ast.Node {
	if callee == nil {
		return nil
	}
	if callee.Kind == ast.KindPropertyAccessExpression {
		return callee.AsPropertyAccessExpression().Name()
	}
	if callee.Kind == ast.KindIdentifier {
		return callee
	}
	return nil
}
