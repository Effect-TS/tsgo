package completions_test

import (
	"strings"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"
)

func selfInClassesItems(t *testing.T, source string, position int) []*lsproto.CompletionItem {
	t.Helper()
	return filterCompletionItems(completionItemsAt(t, source, position), func(item *lsproto.CompletionItem) bool {
		for _, prefix := range []string{"Service<", `Tag("`, "Class<", `make("`} {
			if strings.HasPrefix(item.Label, prefix) {
				return true
			}
		}
		return false
	})
}

func TestSelfInClassesCompletions_OnlyInNamedClassExtends(t *testing.T) {
	t.Parallel()

	type testCase struct {
		name     string
		source   string
		position int
		want     bool
	}
	cases := []testCase{
		{name: "empty source", source: ""},
		{name: "interface extends", source: `interface Foo extends Bar`},
	}
	for _, provider := range []struct {
		name          string
		directive     string
		namespaceImp  string
		access        string
		namedImport   string
		importedIdent string
	}{
		{"effect", "// @effect-v3\n", `import * as Effect from "effect/Effect"`, "Effect.Service", `import { Effect } from "effect"`, "Effect"},
		{"context", "", `import * as Context from "effect/Context"`, "Context.Tag", `import { Context } from "effect"`, "Context"},
		{"sqlModel", "", `import * as Model from "@effect/sql/Model"`, "Model.Class", `import { Model } from "@effect/sql"`, "Model"},
		{"rpc", "// @effect-v3\n", `import * as Rpc from "@effect/rpc/Rpc"`, "Rpc.make", `import { Rpc } from "@effect/rpc"`, "Rpc"},
	} {
		namespace := provider.access[:strings.Index(provider.access, ".")+1]
		header := provider.directive + provider.namespaceImp
		namedImport := provider.directive + provider.namedImport
		cases = append(cases,
			testCase{name: provider.name + "/named class extends", source: header + "\nclass Foo extends " + provider.access, want: true},
			testCase{name: provider.name + "/variable initializer", source: header + "\nconst x = " + provider.access},
			testCase{name: provider.name + "/member access outside class", source: header + "\nconst x = " + namespace},
			testCase{name: provider.name + "/anonymous class", source: header + "\nconst x = class extends " + provider.access},
			testCase{name: provider.name + "/inside import declaration", source: namedImport, position: strings.Index(namedImport, provider.importedIdent) + len(provider.importedIdent)},
		)
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			position := tc.position
			if position == 0 {
				position = len(tc.source)
			}
			items := selfInClassesItems(t, tc.source, position)
			if tc.want && len(items) == 0 {
				t.Errorf("expected self-in-classes completions for %q", tc.source)
			}
			if !tc.want && items != nil {
				t.Errorf("expected no self-in-classes completions for %q, got %d items", tc.source, len(items))
			}
		})
	}
}
