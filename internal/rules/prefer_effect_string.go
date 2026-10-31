package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

var PreferEffectString = rule.Rule{
	Name:            "preferEffectString",
	Group:           "effectNative",
	Description:     "Suggests Effect String APIs for native String method invocations, with semantic caveats",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Consider_effect_SlashString_0_instead_of_native_String_1_2_effect_preferEffectString.Code()},
	Run:             runPreferEffectString,
}

type stringMethodPreference struct {
	nativeName string
	effectAPI  string
	caveat     string
}

var stringMethodPreferences = []stringMethodPreference{
	{"at", "at", "Returns Option, with None replacing undefined."},
	{"charAt", "charAt", "Returns Option, with None replacing the empty string for an out-of-range index."},
	{"charCodeAt", "charCodeAt", "Returns Option<number>, with None replacing NaN."},
	{"codePointAt", "codePointAt", "Returns Option<number>, with None replacing undefined."},
	{"concat", "concat", "Accepts exactly one string argument; native concat accepts any number of arguments and converts non-strings."},
	{"endsWith", "endsWith", "Same runtime behavior."},
	{"includes", "includes", "Same runtime behavior."},
	{"indexOf", "indexOf", "Returns Option<number>, with None replacing -1; there is no position parameter."},
	{"lastIndexOf", "lastIndexOf", "Returns Option<number>, with None replacing -1; there is no position parameter."},
	{"localeCompare", "localeCompare", "Returns an Ordering of -1, 0, or 1; Effect v4 types locales as Array<string>, so a single locale string needs an array."},
	{"match", "match", "Returns Option, with None replacing null."},
	{"matchAll", "matchAll", "Requires a RegExp; string patterns are not accepted."},
	{"normalize", "normalize", "Same runtime behavior."},
	{"padEnd", "padEnd", "Same runtime behavior."},
	{"padStart", "padStart", "Same runtime behavior."},
	{"repeat", "repeat", "In Effect v4, a negative count returns an empty string instead of throwing RangeError."},
	{"replace", "replace", "Requires a string replacement; replacer callbacks are not accepted."},
	{"replaceAll", "replaceAll", "Requires a string replacement; replacer callbacks are not accepted."},
	{"search", "search", "Returns Option<number>, with None replacing -1."},
	{"slice", "slice", "Same runtime behavior."},
	{"split", "split", "Returns a NonEmptyArray, so splitting an empty string by \"\" returns [\"\"] instead of []; there is no limit parameter."},
	{"startsWith", "startsWith", "Same runtime behavior."},
	{"substring", "substring", "Same runtime behavior."},
	{"toLocaleLowerCase", "toLocaleLowerCase", "Same runtime behavior; Effect v4 types the locale as string or Array<string>, without Intl.Locale."},
	{"toLocaleUpperCase", "toLocaleUpperCase", "Same runtime behavior; Effect v4 types the locale as string or Array<string>, without Intl.Locale."},
	{"toLowerCase", "toLowerCase", "Same runtime behavior; literal inputs keep a Lowercase<T> type."},
	{"toUpperCase", "toUpperCase", "Same runtime behavior; literal inputs keep an Uppercase<T> type."},
	{"trim", "trim", "Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r."},
	{"trimStart", "trimStart", "Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r."},
	{"trimEnd", "trimEnd", "Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r."},
	{"trimLeft", "trimStart", "Deprecated alias of trimStart; the literal result type only removes spaces, \\n, \\t, and \\r."},
	{"trimRight", "trimEnd", "Deprecated alias of trimEnd; the literal result type only removes spaces, \\n, \\t, and \\r."},
}

var stringMethodNames = func() map[string]bool {
	names := make(map[string]bool, len(stringMethodPreferences))
	for _, preference := range stringMethodPreferences {
		names[preference.nativeName] = true
	}
	return names
}()

func runPreferEffectString(ctx *rule.Context) []*ast.Diagnostic {
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
		if parent != nil && parent.Kind == ast.KindCallExpression && ast.SkipParentheses(parent.AsCallExpression().Expression) == node &&
			name != nil && stringMethodNames[name.Text()] && ast.SkipParentheses(receiver).Kind != ast.KindSuperKeyword {
			for i := range stringMethodPreferences {
				preference := &stringMethodPreferences[i]
				if ctx.TypeParser.IsNativeStringMethodReference(node, preference.nativeName) {
					diagnostics = append(diagnostics, ctx.NewDiagnostic(
						ctx.SourceFile, ctx.GetErrorRange(name),
						tsdiag.Consider_effect_SlashString_0_instead_of_native_String_1_2_effect_preferEffectString,
						nil, preference.effectAPI, preference.nativeName, preference.caveat,
					))
					break
				}
			}
		}
		//ets:ast-walk finds native String method accesses used as callees, including zero-argument calls
		node.ForEachChild(walk)
		return false
	}
	walk(ctx.SourceFile.AsNode())
	return diagnostics
}
