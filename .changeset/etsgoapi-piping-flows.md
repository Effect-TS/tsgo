---
"@effect/tsgo": minor
---

Expose piping-flow extraction through the `etsgoapi` Go API.

The type parser already understands "piping flows" — a subject expression followed by an ordered list of transformations, unifying the `pipe(...)`, pipeable `.pipe(...)`, data-first, data-last and `Effect.fn` forms — but that analysis was only reachable from internal packages. `etsgoapi.TypeParser` now surfaces it so external Go integrations can consume it without importing `internal/...`.

```go
tp := etsgoapi.NewTypeParser(program, checker)
for _, flow := range tp.PipingFlows(sourceFile, true /* includeEffectFn */) {
	// flow.Subject.Node / flow.Subject.OutType — the starting expression and its type
	for _, step := range flow.Transformations {
		// step.Kind (pipe | pipeable | dataFirst | dataLast | call | effectFn | effectFnUntraced)
		// step.Callee / step.Args / step.OutType
	}
}
```

Adds the public `PipingFlow`, `PipingFlowSubject`, `PipingFlowTransformation` and `TransformationKind` types alongside the new `(*TypeParser).PipingFlows` method.
