package rules

import "testing"

func TestPreferEffectStringMethods(t *testing.T) {
	t.Parallel()
	for _, tc := range []struct {
		source string
		want   string
	}{
		{"\"hello\".at(0);", "Consider effect/String.at instead of native String.at. Returns Option, with None replacing undefined. effect(preferEffectString)"},
		{"\"hello\".charAt(0);", "Consider effect/String.charAt instead of native String.charAt. Returns Option, with None replacing the empty string for an out-of-range index. effect(preferEffectString)"},
		{"\"hello\".charCodeAt(0);", "Consider effect/String.charCodeAt instead of native String.charCodeAt. Returns Option<number>, with None replacing NaN. effect(preferEffectString)"},
		{"\"hello\".codePointAt(0);", "Consider effect/String.codePointAt instead of native String.codePointAt. Returns Option<number>, with None replacing undefined. effect(preferEffectString)"},
		{"\"hello\".concat(\"!\");", "Consider effect/String.concat instead of native String.concat. Accepts exactly one string argument; native concat accepts any number of arguments and converts non-strings. effect(preferEffectString)"},
		{"\"hello\".endsWith(\"o\");", "Consider effect/String.endsWith instead of native String.endsWith. Same runtime behavior. effect(preferEffectString)"},
		{"\"hello\".includes(\"e\");", "Consider effect/String.includes instead of native String.includes. Same runtime behavior. effect(preferEffectString)"},
		{"\"hello\".indexOf(\"l\");", "Consider effect/String.indexOf instead of native String.indexOf. Returns Option<number>, with None replacing -1; there is no position parameter. effect(preferEffectString)"},
		{"\"hello\".lastIndexOf(\"l\");", "Consider effect/String.lastIndexOf instead of native String.lastIndexOf. Returns Option<number>, with None replacing -1; there is no position parameter. effect(preferEffectString)"},
		{"\"hello\".localeCompare(\"world\");", "Consider effect/String.localeCompare instead of native String.localeCompare. Returns an Ordering of -1, 0, or 1; Effect v4 types locales as Array<string>, so a single locale string needs an array. effect(preferEffectString)"},
		{"\"hello\".match(/l/);", "Consider effect/String.match instead of native String.match. Returns Option, with None replacing null. effect(preferEffectString)"},
		{"\"hello\".matchAll(/l/g);", "Consider effect/String.matchAll instead of native String.matchAll. Requires a RegExp; string patterns are not accepted. effect(preferEffectString)"},
		{"\"hello\".normalize();", "Consider effect/String.normalize instead of native String.normalize. Same runtime behavior. effect(preferEffectString)"},
		{"\"hello\".padEnd(8);", "Consider effect/String.padEnd instead of native String.padEnd. Same runtime behavior. effect(preferEffectString)"},
		{"\"hello\".padStart(8);", "Consider effect/String.padStart instead of native String.padStart. Same runtime behavior. effect(preferEffectString)"},
		{"\"hello\".repeat(2);", "Consider effect/String.repeat instead of native String.repeat. In Effect v4, a negative count returns an empty string instead of throwing RangeError. effect(preferEffectString)"},
		{"\"hello\".replace(\"l\", \"r\");", "Consider effect/String.replace instead of native String.replace. Requires a string replacement; replacer callbacks are not accepted. effect(preferEffectString)"},
		{"\"hello\".replaceAll(\"l\", \"r\");", "Consider effect/String.replaceAll instead of native String.replaceAll. Requires a string replacement; replacer callbacks are not accepted. effect(preferEffectString)"},
		{"\"hello\".search(/l/);", "Consider effect/String.search instead of native String.search. Returns Option<number>, with None replacing -1. effect(preferEffectString)"},
		{"\"hello\".slice(1);", "Consider effect/String.slice instead of native String.slice. Same runtime behavior. effect(preferEffectString)"},
		{"\"hello\".split(\"\");", "Consider effect/String.split instead of native String.split. Returns a NonEmptyArray, so splitting an empty string by \"\" returns [\"\"] instead of []; there is no limit parameter. effect(preferEffectString)"},
		{"\"hello\".startsWith(\"h\");", "Consider effect/String.startsWith instead of native String.startsWith. Same runtime behavior. effect(preferEffectString)"},
		{"\"hello\".substring(1);", "Consider effect/String.substring instead of native String.substring. Same runtime behavior. effect(preferEffectString)"},
		{"\"hello\".toLocaleLowerCase();", "Consider effect/String.toLocaleLowerCase instead of native String.toLocaleLowerCase. Same runtime behavior; Effect v4 types the locale as string or Array<string>, without Intl.Locale. effect(preferEffectString)"},
		{"\"hello\".toLocaleUpperCase();", "Consider effect/String.toLocaleUpperCase instead of native String.toLocaleUpperCase. Same runtime behavior; Effect v4 types the locale as string or Array<string>, without Intl.Locale. effect(preferEffectString)"},
		{"\"hello\".toLowerCase();", "Consider effect/String.toLowerCase instead of native String.toLowerCase. Same runtime behavior; literal inputs keep a Lowercase<T> type. effect(preferEffectString)"},
		{"\"hello\".toUpperCase();", "Consider effect/String.toUpperCase instead of native String.toUpperCase. Same runtime behavior; literal inputs keep an Uppercase<T> type. effect(preferEffectString)"},
		{"\"hello\".trim();", "Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"},
		{"\"hello\".trimStart();", "Consider effect/String.trimStart instead of native String.trimStart. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"},
		{"\"hello\".trimEnd();", "Consider effect/String.trimEnd instead of native String.trimEnd. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"},
		{"\"hello\".trimLeft();", "Consider effect/String.trimStart instead of native String.trimLeft. Deprecated alias of trimStart; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"},
		{"\"hello\".trimRight();", "Consider effect/String.trimEnd instead of native String.trimRight. Deprecated alias of trimEnd; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"},
	} {
		t.Run(tc.source, func(t *testing.T) {
			t.Parallel()
			ctx, done := compileApiStabilityFiles(t, map[string]string{"test.ts": tc.source})
			defer done()
			diagnostics := runPreferEffectString(ctx)
			if len(diagnostics) != 1 || diagnostics[0].String() != tc.want {
				t.Fatalf("diagnostics = %v, want one %q", diagnostics, tc.want)
			}
		})
	}
}

func TestPreferEffectStringInvocationOnly(t *testing.T) {
	t.Parallel()
	ctx, done := compileApiStabilityFiles(t, map[string]string{"test.ts": `
const s = "hello";
const f = s.trim;
[s].map(s.trim);
String.prototype.trim.call(s);
s.substr(1);
s.toString();
s.anchor("id");
s.isWellFormed();
(s.trim)();
s.trim?.();
`})
	defer done()
	diagnostics := runPreferEffectString(ctx)
	const want = "Consider effect/String.trim instead of native String.trim. Same runtime behavior; the literal result type only removes spaces, \\n, \\t, and \\r. effect(preferEffectString)"
	if len(diagnostics) != 2 {
		t.Fatalf("diagnostics = %v, want two trim invocations", diagnostics)
	}
	for _, diagnostic := range diagnostics {
		if diagnostic.String() != want {
			t.Fatalf("diagnostic = %q, want %q", diagnostic.String(), want)
		}
	}
}
