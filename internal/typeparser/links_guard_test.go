package typeparser

import (
	"reflect"
	"strings"
	"testing"
)

// TestEffectLinksClassified makes every EffectLinks field declare whether
// BeginTransientAnalysis clears it, so a new cache cannot silently outlive
// (or be dropped from) the CLI release.
func TestEffectLinksClassified(t *testing.T) {
	t.Parallel()
	cleared := setOf(
		"TypeAtLocation", "ReferenceSymbol", "EffectGenCall", "EffectFnCall", "ParseEffectFnOpportunity",
		"ParsePipeCall", "ExecutionFlow", "EffectContextFlags", "EffectYieldGeneratorFunction",
		"EffectContextAnalyzed", "ExpectedAndRealTypes", "ApiStabilityUsages", "PipingFlowsWithEffectFn",
		"PipingFlowsWithoutEffectFn",
	)
	kept := setOf(
		"transientAnalysisDepth", "EffectType", "StreamType", "StrictEffectType", "EffectSubtype", "FiberType",
		"EffectYieldableType", "HasEffectTypeId", "LayerType", "ServiceType", "ContextTag", "EffectSchemaTypes",
		"IsScopeType", "IsPipeableType", "PromiseType", "IsGlobalErrorType", "IsYieldableErrorType",
		"ModuleExportReference", "PipeableSignatureShape", "nativeArrayRegistry", "nativeArrayMember",
		"nativeObjectRegistry", "nativeObjectMember", "ApiStabilityDeclaredSymbol",
		"ApiStabilityDeclaredSignature", "ApiStabilityDeclaredDeclaration", "ApiStabilitySurfaceType",
		"ApiStabilitySurfaceSignature", "ExtendsContextTag", "ExtendsDataTaggedError", "ExtendsEffectModelClass",
		"ExtendsEffectService", "ExtendsEffectTag", "ExtendsSchemaClass", "ExtendsSchemaError",
		"ExtendsSchemaOpaque", "ExtendsSchemaRequestClass", "ExtendsSchemaTaggedClass", "ExtendsSchemaTaggedError",
		"ExtendsSchemaTaggedRequest", "ExtendsServiceMapService", "ExtendsEffectSqlModelClass",
		"discoverPackagesComputed", "discoverPackagesValue", "detectEffectVersionComputed",
		"detectEffectVersionValue", "PackageJsonForSourceFile",
	)

	links := reflect.TypeFor[EffectLinks]()
	fields := map[string]bool{}
	for f := range links.Fields() {
		name := f.Name
		fields[name] = true
		switch {
		case cleared[name] && kept[name]:
			t.Errorf("EffectLinks.%s is classified as both cleared and kept", name)
		case !cleared[name] && !kept[name]:
			t.Errorf("EffectLinks.%s is not classified as cleared or kept by BeginTransientAnalysis", name)
		}
	}
	for _, names := range []map[string]bool{cleared, kept} {
		for name := range names {
			if !fields[name] {
				t.Errorf("EffectLinks.%s is classified but does not exist", name)
			}
		}
	}

	// Populate every exported store with a zero key, then check the release
	// clears exactly the stores classified as cleared.
	tp := &TypeParser{links: &EffectLinks{}}
	v := reflect.ValueOf(tp.links).Elem()
	stores := map[string]reflect.Value{}
	for f := range links.Fields() {
		if f.IsExported() && strings.HasPrefix(f.Type.String(), "core.LinkStore[") {
			stores[f.Name] = v.FieldByIndex(f.Index).Addr()
		}
	}
	finish := tp.BeginTransientAnalysis()
	for _, store := range stores {
		get := store.MethodByName("Get")
		get.Call([]reflect.Value{reflect.Zero(get.Type().In(0))})
	}
	finish()
	for name, store := range stores {
		has := store.MethodByName("Has")
		if got := has.Call([]reflect.Value{reflect.Zero(has.Type().In(0))})[0].Bool(); got == cleared[name] {
			t.Errorf("EffectLinks.%s: entry survived = %t, want %t", name, got, !cleared[name])
		}
	}
}

func setOf(names ...string) map[string]bool {
	set := make(map[string]bool, len(names))
	for _, name := range names {
		set[name] = true
	}
	return set
}
