package completions_test

import (
	"strings"
	"testing"
)

func TestServiceMapSelfInClasses_NamespaceImport(t *testing.T) {
	t.Parallel()

	source := `import * as ServiceMap from "effect/ServiceMap"

export class MyService extends ServiceMap.`
	items := serviceMapSelfInClassesItemsWithPackageJSON(t, source, len(source))
	if len(items) != 2 {
		t.Fatalf("expected 2 items, got %d", len(items))
	}

	if items[0].Label != "Service<MyService, {}>" {
		t.Errorf("item[0].Label = %q, want %q", items[0].Label, "Service<MyService, {}>")
	}
	if got := items[0].TextEdit.TextEdit.NewText; !strings.HasPrefix(got, `ServiceMap.Service<MyService, {${0}}>()("`) || !strings.HasSuffix(got, `"){}`) {
		t.Errorf("item[0].insertText = %q", got)
	}

	if items[1].Label != "Service<MyService>({ make })" {
		t.Errorf("item[1].Label = %q, want %q", items[1].Label, "Service<MyService>({ make })")
	}
	if got := items[1].TextEdit.TextEdit.NewText; got != `ServiceMap.Service<MyService>()("@effect/harness-effect-v4/test/MyService", { make: ${0} }){}` {
		t.Errorf("item[1].insertText = %q", got)
	}
}

func TestServiceMapSelfInClasses_DirectImport(t *testing.T) {
	t.Parallel()

	source := `import { Service } from "effect/ServiceMap"

export class MyService extends Service`
	items := serviceMapSelfInClassesItemsWithPackageJSON(t, source, len(source))
	if len(items) != 2 {
		t.Fatalf("expected 2 items, got %d", len(items))
	}

	if got := items[0].TextEdit.TextEdit.NewText; !strings.HasPrefix(got, `Service<MyService, {${0}}>()("`) || !strings.HasSuffix(got, `"){}`) {
		t.Errorf("item[0].insertText = %q", got)
	}
	if got := items[1].TextEdit.TextEdit.NewText; got != `Service<MyService>()("@effect/harness-effect-v4/test/MyService", { make: ${0} }){}` {
		t.Errorf("item[1].insertText = %q", got)
	}
}

func TestServiceMapSelfInClasses_IdentifierKeyPattern(t *testing.T) {
	t.Parallel()

	source := `// @test-config { "keyPatterns": [ { "pattern": "package-identifier", "target": "service" } ] }
import * as ServiceMap from "effect/ServiceMap"

export class MyService extends ServiceMap.`
	items := serviceMapSelfInClassesItemsWithPackageJSON(t, source, len(source))
	if len(items) != 2 {
		t.Fatalf("expected 2 items, got %d", len(items))
	}

	for i, item := range items {
		if !strings.Contains(item.TextEdit.TextEdit.NewText, `"@effect/harness-effect-v4/test/MyService"`) {
			t.Errorf("item[%d].insertText = %q, want package identifier key", i, item.TextEdit.TextEdit.NewText)
		}
	}
}

func TestServiceMapSelfInClasses_MiddleOfIdentifier(t *testing.T) {
	t.Parallel()

	source := `import { Effect, ServiceMap, Stream } from "effect"

class Foo extends ServiceMap.S

Stream.unwrap(Effect.gen(function*() {
	const a = yield* Foo

	return Stream.succeed(a.count)
}))`
	position := strings.Index(source, "ServiceMap.S") + len("ServiceMap.S")
	items := serviceMapSelfInClassesItemsWithPackageJSON(t, source, position)
	if len(items) != 2 {
		t.Fatalf("expected 2 items, got %d", len(items))
	}

	if items[0].Label != "Service<Foo, {}>" {
		t.Errorf("item[0].Label = %q, want %q", items[0].Label, "Service<Foo, {}>")
	}
	if items[1].Label != "Service<Foo>({ make })" {
		t.Errorf("item[1].Label = %q, want %q", items[1].Label, "Service<Foo>({ make })")
	}
	if got := items[0].TextEdit.TextEdit.NewText; !strings.HasPrefix(got, `ServiceMap.Service<Foo, {${0}}>()("`) || !strings.HasSuffix(got, `"){}`) {
		t.Errorf("item[0].insertText = %q", got)
	}
}
