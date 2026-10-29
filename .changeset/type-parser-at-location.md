---
"@effect/tsgo": patch
---

Refactor the internal type parser API around parse methods. Rules no longer pair a syntactic "can this node be an Effect library type" guard with `GetTypeAtLocation` by hand: new `XxxAtLocation(node)` parsers (`EffectTypeAtLocation`, `StrictEffectTypeAtLocation`, `LayerTypeAtLocation`, `StreamTypeAtLocation`, `EffectSchemaTypesAtLocation`, `ServiceTypeAtLocation`, `ContextTagAtLocation`, `EffectYieldableTypeAtLocation`, `HasEffectTypeIdAtLocation`) skip computing the type of literals, functions and other nodes that can never be one, and the guard is now private.

```go
// before
if tp.NodeCannotBeEffectLibraryType(arg) {
	continue
}
argType := tp.GetTypeAtLocation(arg)
if argType == nil || tp.LayerType(argType) == nil {
	continue
}

// after
if tp.LayerTypeAtLocation(arg) == nil {
	continue
}
```

The boolean wrappers `IsEffectType`, `StrictIsEffectType`, `IsEffectSubtype`, `IsFiberType`, `IsLayerType`, `IsServiceType`, `IsContextTag` and `IsSchemaType` are removed in favour of their parse methods, and `IsArrayType`, `IsScopeType` and `IsYieldableErrorType` become `ArrayType` (returning the element type), `ScopeType` and `YieldableErrorType`. A new `typeatlocation` custom lint reports parsing the result of `GetTypeAtLocation` where an `AtLocation` parser can be used instead. Diagnostics are unchanged; rules that previously typed every candidate node now skip the ones that cannot match.
