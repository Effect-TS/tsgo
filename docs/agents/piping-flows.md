# Piping flows

A **piping flow** is the parser's single shape for "a value passed through transformations", however the source spells it. `pipe(x, f, g)`, `x.pipe(f, g)`, `g(f(x))`, `Effect.map(x, h)` and `Effect.fn(body, f)` all become a **subject** followed by ordered **transformations**. A rule that matches the flow catches every spelling at once; a rule that matches one AST shape catches only that spelling. That is why rules start from flows.

API: `internal/typeparser/piping_flow.go`. Rewriter edits: `internal/rewriter/piping_flow.go`.

## Normalization by kind

Each step has a `Kind()`, a `Callee()`, `Args()` (empty for a bare operator) and an `OutType()`. These examples are copied from `testdata/baselines/reference/effect-v4/*.pipings.txt`.

| Source | Subject | Steps (kind, callee, args) |
| --- | --- | --- |
| `pipe(tag, Effect.fail)` | `tag` | `pipe` `Effect.fail` () |
| `pipe(failable, Effect.catch(Effect.die))` | `failable` | `pipe` `Effect.catch` [`Effect.die`] |
| `program.pipe(withLive)` | `program` | `pipeable` `withLive` () |
| `loose.pipe(Effect.as(n))` | `loose` | `pipeable` `Effect.as` [`n`] |
| `f(1)` | `1` | `call` `f` () |
| `Layer.effect(Service)(acquisition)` (curried) | `acquisition` | `call` `Layer.effect` [`Service`] |
| `Effect.gen(function*() { ... })` | the generator function | `call` `Effect.gen` () |
| `Effect.fn("name")(function*() { ... })` | the generator function | `call` `Effect.fn("name")` () |
| `Effect.map(e, (n) => n * 2)` | `e` | `dataFirst` `Effect.map` [`(n) => n * 2`] |
| `Layer.effect(Cache, Cache)` | `Cache` (the last argument) | `dataLast` `Layer.effect` [`Cache`] |
| `fetch(url, { signal })` | `url` | `callWithOptionals` `fetch` [`{ signal }`] |
| `Effect.fn(() => ..., Effect.map(f))` | the whole `Effect.fn(...)` call | `effectFn` `Effect.map` [`f`] |
| `Effect.fnUntraced(function*() {...}, Effect.map(f))` | the whole call | `effectFnUntraced` `Effect.map` [`f`] |

Flattening joins nested and chained forms into one flow:

- `addOne(5).pipe(double, toString)` has subject `5` and steps `call addOne`, `pipeable double`, `pipeable toString`.
- `Effect.succeed(1).pipe(Effect.asVoid).pipe(Effect.as("done"))` has subject `1` and steps `call Effect.succeed`, `pipeable Effect.asVoid`, `pipeable Effect.as ["done"]`. A pipe boundary leaves no trace, so a rule about the `.pipe()` calls themselves (`unnecessaryPipeChain`) needs `ParsePipeCall`, not flows.

Every argument of a step, every extra argument of a `dataFirst` or `callWithOptionals` call, and every `Effect.fn` body is searched for flows of its own. A source step belongs to exactly one flow returned by `PipingFlows`.

### What never becomes a step

A rule about these shapes needs another tool. An AST walk is fine here; see `CODING_STANDARDS.md`.

- Calls with zero arguments: `Math.random()`, `value.trim()`, the `()` that invokes an Effect.fn IIFE.
- Calls to a callee with a rest parameter (`console.log`, `Layer.mergeAll`, `Schema.Union(a, b)`) or with several overloads (`JSON.stringify(v, null, 2)`, `Effect.gen(self, fn)`).
- Calls with two or more arguments whose callee is neither dual (data-first or data-last) nor a single signature with one required parameter.
- Nodes that are not calls: `new` expressions, classes, type nodes, `yield`, `return`, `try`, `instanceof`, property accesses, object literals.
- An empty pipe: `x.pipe()` adds no step.

## Trying a shape

Before matching a shape, run it through the parser:

```sh
pnpm exec repoctl pipings - --errors <<'EOF'
import { Effect } from "effect"
export const program = Effect.sync(() => Promise.resolve(1)).pipe(Effect.asVoid)
EOF
```

This prints every flow in the snippet, in the `.pipings.txt` format above. `--errors` adds the diagnostics, which shows whether a rule already fires. Use `--effect v3` to check against Effect v3, and a file path instead of `-` to read from a file. The snippet can use `// @filename:` units, as fixtures do. Try each spelling the rule must catch: if one does not come out as the flow you expect, that is a primitive gap, not a reason to walk the AST.

## Finding flows

- `PipingFlows(sf, includeEffectFn)` returns every complete flow in the file, cached, in source order. It is the default entry point for a rule. Pass `includeEffectFn = true` unless the rule must ignore `effectFn`/`effectFnUntraced` steps; with `false`, the trailing arguments of an `Effect.fn` are not steps.
- `LongestPipingFlowAt(node, includeEffectFn)` returns the longest flow rooted at an expression the rule already holds, such as a callback body or a step argument, without the enclosing flow. Use it for "the flow inside this argument".
- `ParsePipeCall(node)` parses one `pipe(...)` or `.pipe(...)` call. Use it only for rules about the pipe call itself.

## Matching

- `MatchesPrefix(subject, steps...)`: the subject predicate and the first steps match; more steps may follow.
- `MatchesExactly(subject, steps...)`: the same, with no extra steps allowed.
- `FindTransformationSequences(steps...)`: every run of consecutive matching steps, overlaps included. `Start()` is the index of the first step.
- `CopyPrefix(n)`: a flow of the subject and the first `n` steps, to match or rewrite a prefix.

Match a step on its callee with the `IsNodeReferenceTo…` helpers, for example `tp.IsNodeReferenceToEffectModuleApi(t.Callee(), "sync")`, and on `Args().Len()`. A callee check already excludes every other kind, so there is no need to also switch on `Kind()`; `Effect.sync(thunk)` and `pipe(thunk, Effect.sync)` match the same predicate. `internal/rules/lazy_promise_in_effect_sync.go` is a short example of the whole pattern.

## A step's input

- `TransformationInputType(i)` is the type flowing into step `i`: the subject's type for `i == 0`, else the previous step's `OutType()`. It works for every kind; it is nil only when the checker has no type.
- `TransformationInputNode(i)` is the expression feeding step `i` **only when it exists in the source on its own**: the subject for step 0, a `call` step's single argument, a `dataFirst`/`callWithOptionals` call's first argument, a `dataLast` call's last argument. For a `pipe` or `pipeable` step after index 0 it is **nil**, because the input is the previous step's result, not a node. A rule that skips nil here silently misses every `.pipe(...)` spelling. Reason over the type instead, or match the previous step. If the rule truly needs the node, raise it as a primitive gap (see `CODING_STANDARDS.md`).

## Rewriting a flow

These live on `*rewriter.Tracker` and keep every step outside the edit untouched:

| Change | Edit |
| --- | --- |
| Replace the subject and the first `n` steps with one expression (`Effect.suspend(() => Effect.fail(e))` becomes `Effect.failSync(() => e)`) | `ReplacePipingFlowPrefix` |
| Swap one step for another operator, keeping how its input is applied (pipe argument, data-first, data-last, curried, call with optionals) | `ReplacePipingFlowTransformation` |
| Merge adjacent steps `start..end` into one operator (`map(f)` then `mapError(g)` becomes `mapBoth`) | `CollapsePipingFlowTransformations` |
| Drop the last step | `RemoveTrailingPipingFlowTransformation` |

`ReplacePipingFlowPrefix` refuses a prefix that ends in an `Effect.fn` step and returns `false`; check the result. A replacement operator is a `PipingFlowTransformationReplacement`: nil `Arguments` is a bare operator (`Effect.asVoid`), a non-nil list is a call (`Effect.as("done")`). `CollapsePipingFlowTransformations` carries comments over to the replacement; the others reprint the nodes they move.

## Known gaps

Record new ones here once the user agrees they belong in the parser or rewriter.

- `TransformationInputNode` has no node for `pipe`/`pipeable` steps after index 0 (above).
- A step does not record instantiation type arguments or optional-chain (`?.`) calls.
- The rewriter's lookup of a step's enclosing pipe call is not exported.
- Edits other than `CollapsePipingFlowTransformations` may drop comments or re-indent the nodes they move.
