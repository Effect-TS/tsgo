package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

var PreferEffectRecord = rule.Rule{
	Name:            "preferEffectRecord",
	Group:           "effectNative",
	Description:     "Suggests Effect Record APIs for native Object member references, with semantic caveats",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Consider_effect_SlashRecord_0_instead_of_native_1_2_effect_preferEffectRecord.Code()},
	Run:             runPreferEffectRecord,
}

type recordMemberNamespace uint8

const (
	recordInstanceMember recordMemberNamespace = iota
	recordConstructorMember
)

type recordMethodPreference struct {
	namespace  recordMemberNamespace
	nativeName string
	effectAPI  string
	caveat     string
}

var recordMethodPreferences = []recordMethodPreference{
	{recordConstructorMember, "keys", "keys", "Requires a record rather than arrays or primitives; types keys as the inferred key union, which does not guarantee exact runtime keys."},
	{recordConstructorMember, "values", "values", "Requires a record rather than arrays or primitives; snapshots keys before reads, so getter or proxy mutations can change the result."},
	{recordConstructorMember, "entries", "toEntries", "Requires a record rather than arrays or primitives; types entries with inferred keys and snapshots keys before reads, so getter or proxy mutations can change the result."},
	{recordConstructorMember, "fromEntries", "fromEntries", "Requires typed string or symbol key tuples; native numeric keys and loose entry arrays need adaptation."},
	{recordConstructorMember, "hasOwn", "has", "Requires a record rather than arrays or primitives and a string or symbol key of that record's key type; arbitrary strings may need narrowing."},
	{recordInstanceMember, "hasOwnProperty", "has", "Pass the receiver explicitly; requires a record rather than arrays or primitives and a string or symbol key of that record's key type. Avoids instance binding and overrides."},
}

var recordMethodNames = func() map[string]bool {
	names := make(map[string]bool, len(recordMethodPreferences))
	for _, preference := range recordMethodPreferences {
		names[preference.nativeName] = true
	}
	return names
}()

func (p *recordMethodPreference) matches(tp *typeparser.TypeParser, node *ast.Node) bool {
	if p.namespace == recordConstructorMember {
		return tp.IsNativeObjectConstructorMethodReference(node, p.nativeName)
	}
	return tp.IsNativeObjectMethodReference(node, p.nativeName)
}

func (p *recordMethodPreference) nativeLabel() string {
	if p.namespace == recordInstanceMember {
		return "Object.prototype." + p.nativeName
	}
	return "Object." + p.nativeName
}

func runPreferEffectRecord(ctx *rule.Context) []*ast.Diagnostic {
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
		if name != nil && recordMethodNames[name.Text()] && ast.SkipParentheses(receiver).Kind != ast.KindSuperKeyword &&
			!ast.IsAssignmentTarget(node) && !ast.IsDeclarationName(node) &&
			(parent == nil || parent.Kind != ast.KindDeleteExpression) {
			for i := range recordMethodPreferences {
				preference := &recordMethodPreferences[i]
				if preference.matches(ctx.TypeParser, node) {
					diagnostics = append(diagnostics, ctx.NewDiagnostic(
						ctx.SourceFile, ctx.GetErrorRange(name),
						tsdiag.Consider_effect_SlashRecord_0_instead_of_native_1_2_effect_preferEffectRecord,
						nil, preference.effectAPI, preference.nativeLabel(), preference.caveat,
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
