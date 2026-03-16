# Effect Links — Type Parser Cache

## Goal

Eliminate redundant type parsing across diagnostic rules by caching parsed type results on the checker, so that when multiple rules parse the same type (e.g., `EffectType` on the same `*checker.Type`), the computation happens only once.

## Problem

The current architecture runs 49+ diagnostic rules sequentially per source file. Many rules call the same type parser functions (e.g., `EffectType`, `LayerType`, `ServiceType`) on the same types. Each call re-executes expensive checker API operations (`GetPropertiesOfType`, `GetSignaturesOfType`, variance struct extraction, etc.) with no memoization. This causes a ~10x performance regression compared to standard tsgo.

## Design

### Patch: `any` field on the checker

A minimal patch adds a single untyped field to the tsgo checker:

- Field name: `EffectLinks`
- Field type: `any`
- No import of Effect types in the patch — the concrete structure lives entirely in the Effect codebase.

### Effect side: `EffectLinks` struct and accessor

The Effect codebase defines:

1. **Reuse `core.LinkStore`** — the existing `core.LinkStore[K, V]` from `typescript-go/internal/core` (available via the `shim/core` package) is used directly. No custom link store type is defined in the Effect codebase.
2. A `Cached` helper function that encapsulates the try/compute/store pattern using `Has` + `TryGet` + `Get` from `core.LinkStore`.
3. A concrete `EffectLinks` struct with one `core.LinkStore` field per cached type parser.
4. An accessor function (e.g., `GetEffectLinks(c *checker.Checker) *EffectLinks`) that:
   - Reads `c.EffectLinks` (the `any` field).
   - If nil, creates a new `EffectLinks` instance and assigns it to `c.EffectLinks`.
   - Type-asserts and returns the concrete `*EffectLinks`.

### Cached helper pattern

Each type parser delegates to `LinkStoreCached` (defined in `etscore`), which eliminates boilerplate and ensures consistent cache behavior (including negative caching) across all parsers. The actual parsing logic moves to an internal `compute*` function passed as the closure.

`LinkStoreCached` is generic over both the key and value types (`[K comparable, V any]`), matching `core.LinkStore`'s own type parameters. It lives in the `etscore` package so it can be reused by any package that works with `core.LinkStore`, not just `typeparser`. Go infers `K` and `V` from the `store` argument, so call sites remain concise. This also allows reuse with different key types (e.g., `*ast.Node`) if needed in the future.

### Link store semantics — using `core.LinkStore`

`core.LinkStore[K, V]` stores `map[K]*V` with pool allocation. Its API:
- `Get(key)` — returns `*V`, auto-creates a zero-value entry from pool on miss.
- `TryGet(key)` — returns `*V` or nil (nil = not in cache).
- `Has(key)` — returns true if key is present.

Each link store is keyed by identity pointer (`*checker.Type` or `*ast.Node`). A store entry can hold:

- A positive result (the parsed struct, e.g., `*Effect`, `*Layer`, `*Service`, `true`).
- A negative result (the type was checked and is NOT the target type).

**Negative caching**: Since `core.LinkStore` stores `*V` and `TryGet` returns nil for misses, negative results for pointer-typed parsers cannot be represented by a nil pointer alone. Each cached value type must allow the `Cached` helper to distinguish "not yet computed" (key absent, `Has` = false) from "computed as negative" (key present, `Has` = true, value indicates negative). The `Has` method is the primary presence check; the value at `TryGet`/`Get` carries the result.

### Lazy initialization

`EffectLinks` is nil for non-Effect projects (zero overhead). The accessor creates the struct on first access. This keeps cost at zero for projects that don't trigger Effect diagnostics.

## Type parsers to cache

All exported type parser functions that take a `*checker.Type` and return a parsed result:

### Effect type parsers (keyed by `*checker.Type`)
- `EffectType` → `*Effect`
- `StrictEffectType` → `*Effect`
- `EffectSubtype` → `*Effect`
- `FiberType` → `*Effect`
- `EffectYieldableType` → `*Effect`
- `HasEffectTypeId` → `bool`

### Layer, Service, Schema, Scope parsers (keyed by `*checker.Type`)
- `LayerType` → `*Layer`
- `ServiceType` → `*Service`
- `ContextTag` → `*Service`
- `IsSchemaType` → `bool`
- `EffectSchemaTypes` → `*SchemaTypes`
- `IsScopeType` → `bool`
- `IsPipeableType` → `bool`
- `IsGlobalErrorType` → `bool`
- `IsYieldableErrorType` → `bool`

### Boolean wrappers

`IsEffectType`, `IsLayerType`, `IsServiceType`, `IsContextTag`, `StrictIsEffectType`, `IsEffectSubtype`, `IsFiberType` are thin wrappers around the struct-returning parsers. Once the struct parser is cached, the boolean wrappers automatically benefit — no separate cache needed.

### Node-keyed parsers (keyed by `*ast.Node`)

#### Extends* parsers
- `ExtendsContextTag` → `*ContextTagResult`
- `ExtendsDataTaggedError` → `*DataTaggedErrorResult`
- `ExtendsEffectModelClass` → `*EffectModelClassResult`
- `ExtendsEffectService` → `*EffectServiceResult`
- `ExtendsEffectTag` → `*EffectTagResult`
- `ExtendsSchemaClass` → `*SchemaClassResult`
- `ExtendsSchemaRequestClass` → `*SchemaClassResult`
- `ExtendsSchemaTaggedClass` → `*SchemaTaggedResult`
- `ExtendsSchemaTaggedError` → `*SchemaTaggedResult`
- `ExtendsSchemaTaggedRequest` → `*SchemaTaggedResult`
- `ExtendsServiceMapService` → `*ServiceMapServiceResult`
- `ExtendsEffectSqlModelClass` → `*SqlModelClassResult`

#### Call-site parsers
- `EffectGenCall` → `*EffectGenCallResult`
- `EffectFnCall` → `*EffectFnCallResult`
- `EffectFnGenCall` → `*EffectGenCallResult`
- `EffectFnUntracedGenCall` → `*EffectGenCallResult`
- `EffectFnUntracedEagerGenCall` → `*EffectGenCallResult`
- `ParseEffectFnIife` → `*EffectFnIifeResult`
- `ParseEffectFnOpportunity` → `*EffectFnOpportunityResult`
- `ParsePipeCall` → `*ParsedPipeCallResult`
- `FindEnclosingScopes` → `EnclosingScopes`

### SourceFile-keyed parsers (keyed by `*ast.SourceFile`)
- `PipingFlows` (includeEffectFn=true) → `[]*PipingFlow`
- `PipingFlows` (includeEffectFn=false) → `[]*PipingFlow`
- `ExpectedAndRealTypes` → `[]ExpectedAndRealType`

`PipingFlows` uses two separate link stores — one per value of the `includeEffectFn` parameter.

## Non-Goals

- Caching version detection (`SupportedEffectVersion`) — out of scope for now.
- Caching `IsNodeReferenceTo*` functions — these take a composite key (`*ast.Node` + `memberName string`). Revisit if profiling shows need.
- Per-rule timing or profiling instrumentation.

## Acceptance Criteria

1. The tsgo checker patch adds only one `any` field (`EffectLinks`) — no Effect-specific imports.
2. The `EffectLinks` struct and accessor live in the Effect codebase (e.g., `internal/typeparser/`).
3. **No custom `LinkStore` type** — the Effect codebase uses `core.LinkStore` from `shim/core` directly. The custom `internal/typeparser/link_store.go` and its tests must be removed.
4. All listed type parser functions use the cache: check first, compute on miss, store result (including nil).
5. Negative results (type is not an Effect/Layer/etc.) are cached to avoid re-checking.
6. Non-Effect projects pay zero cost (`EffectLinks` stays nil).
7. All existing tests pass with no behavioral changes.
8. Measurable performance improvement on projects that trigger Effect diagnostics.
