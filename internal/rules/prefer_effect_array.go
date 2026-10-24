package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

var PreferEffectArray = rule.Rule{
	Name:            "preferEffectArray",
	Group:           "effectNative",
	Description:     "Suggests Effect Array APIs for native array member references, with semantic caveats",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Consider_effect_SlashArray_0_instead_of_native_Array_1_2_effect_preferEffectArray.Code()},
	Run:             runPreferEffectArray,
}

type arrayMemberKind uint8

const (
	arrayInstanceMember arrayMemberKind = iota
	arrayConstructorMember
)

type arrayMethodPreference struct {
	kind       arrayMemberKind
	nativeName string
	effectAPI  string
	caveat     string
}

var arrayMethodPreferences = []arrayMethodPreference{
	{arrayInstanceMember, "map", "map", "The callback type accepts (value, index), with no array parameter or thisArg."},
	{arrayInstanceMember, "filter", "filter", "The callback type accepts (value, index), with no array parameter or thisArg; sparse-array holes are visited."},
	{arrayInstanceMember, "every", "every", "The callback type accepts (value, index), with no array parameter or thisArg."},
	{arrayInstanceMember, "some", "some", "The callback type accepts (value, index), with no array parameter or thisArg."},
	{arrayInstanceMember, "forEach", "forEach", "The callback type accepts (value, index), with no array parameter or thisArg."},
	{arrayInstanceMember, "flatMap", "flatMap", "Callbacks must return arrays; the callback type accepts (value, index), with no array parameter or thisArg, and sparse-array holes are visited."},
	{arrayInstanceMember, "reduce", "reduce", "An initial value is required before the callback; the callback type omits the array parameter."},
	{arrayInstanceMember, "reduceRight", "reduceRight", "An initial value is required before the callback; the callback type omits the array parameter."},
	{arrayInstanceMember, "find", "findFirst", "Returns Option, with None replacing undefined; the callback type accepts (value, index), with no array parameter or thisArg."},
	{arrayInstanceMember, "findLast", "findLast", "Returns Option, with None replacing undefined; the callback type accepts (value, index), with no array parameter or thisArg."},
	{arrayInstanceMember, "findIndex", "findFirstIndex", "Returns Option<number>, with None replacing -1; the callback type accepts (value, index), with no array parameter or thisArg."},
	{arrayInstanceMember, "findLastIndex", "findLastIndex", "Returns Option<number>, with None replacing -1; the callback type accepts (value, index), with no array parameter or thisArg."},
	{arrayInstanceMember, "join", "join", "Accepts strings and requires an explicit separator."},
	{arrayInstanceMember, "toReversed", "reverse", "Returns a new array, like native toReversed."},
	{arrayInstanceMember, "toSorted", "sort", "Requires an explicit Order instead of native default ordering or a numeric comparator."},
	{arrayConstructorMember, "isArray", "isArray", "Narrows elements to unknown rather than any."},
}

var arrayMethodNames = func() map[string]bool {
	names := make(map[string]bool, len(arrayMethodPreferences))
	for _, preference := range arrayMethodPreferences {
		names[preference.nativeName] = true
	}
	return names
}()

func (p *arrayMethodPreference) matches(tp *typeparser.TypeParser, node *ast.Node) bool {
	if p.kind == arrayConstructorMember {
		return tp.IsNativeArrayConstructorMethodReference(node, p.nativeName)
	}
	return tp.IsNativeArrayMethodReference(node, p.nativeName)
}

func runPreferEffectArray(ctx *rule.Context) []*ast.Diagnostic {
	var diagnostics []*ast.Diagnostic
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if ast.IsTypeNode(node) {
			return false
		}
		var name, receiver *ast.Node
		switch node.Kind {
		case ast.KindPropertyAccessExpression:
			name = node.AsPropertyAccessExpression().Name()
			receiver = node.AsPropertyAccessExpression().Expression
		case ast.KindElementAccessExpression:
			access := node.AsElementAccessExpression()
			if access.ArgumentExpression != nil && ast.IsStringLiteralLike(access.ArgumentExpression) {
				name = access.ArgumentExpression
				receiver = access.Expression
			}
		}
		parent := node.Parent
		for parent != nil && parent.Kind == ast.KindParenthesizedExpression {
			parent = parent.Parent
		}
		if name != nil && arrayMethodNames[name.Text()] && ast.SkipParentheses(receiver).Kind != ast.KindSuperKeyword &&
			!ast.IsAssignmentTarget(node) && !ast.IsDeclarationName(node) &&
			(parent == nil || parent.Kind != ast.KindDeleteExpression) {
			for i := range arrayMethodPreferences {
				preference := &arrayMethodPreferences[i]
				if preference.matches(ctx.TypeParser, node) {
					diagnostics = append(diagnostics, ctx.NewDiagnostic(
						ctx.SourceFile, ctx.GetErrorRange(name),
						tsdiag.Consider_effect_SlashArray_0_instead_of_native_Array_1_2_effect_preferEffectArray,
						nil, preference.effectAPI, preference.nativeName, preference.caveat,
					))
					break
				}
			}
		}
		node.ForEachChild(walk)
		return false
	}
	walk(ctx.SourceFile.AsNode())
	return diagnostics
}
