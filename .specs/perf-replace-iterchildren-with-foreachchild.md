# Performance: Replace IterChildren with ForEachChild

## Goal

Eliminate the largest source of heap allocations in the Effect diagnostic pipeline by migrating all AST traversals from `IterChildren` to `ForEachChild`.

## Problem

Profiling shows `IterChildren` + `invert` account for **178 million allocations (39% of total)** in a full project check. Every `IterChildren()` call allocates 2 closures:

1. The Go `iter.Seq[*Node]` iterator function (captures the node receiver).
2. The `invert(yield)` wrapper that flips boolean semantics (`Visitor` returns `true` to stop vs Go iterators return `false` to stop).

With 40 rules and 3 type parsers all walking the AST via `IterChildren`, this multiplies to billions of allocations across a full project build, creating massive GC pressure. The aggregate allocation count (12B vs 966M for standard tsgo) is the primary cause of the ~4x check time regression.

## Solution

Replace all `IterChildren` usage with `ForEachChild`, the zero-allocation callback pattern that TypeScript-Go's own checker uses internally.

### Before (allocating)

```go
var walk func(n *ast.Node)
walk = func(n *ast.Node) {
    if n == nil { return }
    // ... node processing ...
    for child := range n.IterChildren() {  // 2 closures per node
        walk(child)
    }
}
walk(sf.AsNode())
```

### After (zero allocation)

```go
var walk ast.Visitor
walk = func(n *ast.Node) bool {
    if n == nil { return false }
    // ... node processing ...
    n.ForEachChild(walk)
    return false  // false = continue visiting siblings
}
walk(sf.AsNode())
```

### Boolean semantics

- `ForEachChild`'s `Visitor` returns `true` to **stop early** (break), `false` to **continue**.
- This is the inverse of Go's `yield` convention. The `invert` wrapper exists solely to bridge this gap — by calling `ForEachChild` directly, the wrapper is eliminated.

### Stack-based traversal adaptation

```go
stack := []*ast.Node{sf.AsNode()}
for len(stack) > 0 {
    node := stack[len(stack)-1]
    stack = stack[:len(stack)-1]
    // ... node processing ...
    node.ForEachChild(func(child *ast.Node) bool {
        stack = append(stack, child)
        return false
    })
}
```

## Files to migrate

### Rules (40 files in `internal/rules/`)

All rule files currently using `IterChildren`:

- `any_unknown_in_error_context.go`
- `class_self_mismatch.go`
- `deterministic_keys.go`
- `effect_fn_iife.go`
- `effect_fn_opportunity.go`
- `effect_gen_uses_adapter.go`
- `effect_in_failure.go`
- `effect_map_void.go`
- `effect_succeed_with_void.go`
- `extends_native_error.go`
- `floating_effect.go`
- `generic_effect_services.go`
- `global_error_in_effect_catch.go`
- `global_error_in_effect_failure.go`
- `instance_of_schema.go`
- `layer_merge_all_with_dependencies.go`
- `leaking_requirements.go`
- `missing_effect_service_dependency.go`
- `missing_return_yield_star.go`
- `missing_star_in_yield_effect_gen.go`
- `non_object_effect_service_type.go`
- `outdated_api.go`
- `overridden_schema_constructor.go`
- `prefer_schema_over_json.go`
- `redundant_schema_tag_identifier.go`
- `return_effect_in_gen.go`
- `run_effect_inside_effect.go`
- `schema_struct_with_tag.go`
- `schema_sync_in_effect.go`
- `schema_union_of_literals.go`
- `scope_in_layer_effect.go`
- `service_not_as_class.go`
- `strict_boolean_expressions.go`
- `strict_effect_provide.go`
- `try_catch_in_effect_gen.go`
- `unknown_in_effect_catch.go`
- `unnecessary_effect_gen.go`
- `unnecessary_fail_yieldable_error.go`
- `unnecessary_pipe.go`
- `unnecessary_pipe_chain.go`

### Type parsers (3 files in `internal/typeparser/`)

- `piping_flow.go`
- `expected_and_real_type.go`
- `effect_fn_opportunity.go`

## Acceptance Criteria

1. Zero remaining uses of `IterChildren` in `internal/rules/` and `internal/typeparser/`.
2. All existing tests pass with no behavioral changes.
3. Measurable reduction in aggregate allocation count (target: eliminate ~178M allocations from IterChildren + invert).
