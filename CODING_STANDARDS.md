# Coding standards

Review rules for changes to Effect rules, fixables, the typeparser and the rewriter. Each rule is a judgement call; mechanical rules are enforced by `pnpm lint` (`_tools/customlint`) and the tests, not here. For how piping flows normalize code and which rewriter edit fits which change, read [`docs/agents/piping-flows.md`](docs/agents/piping-flows.md).

## Detection starts from piping flows

A rule finds the calls it reports by iterating piping flows: `PipingFlows`, then `MatchesPrefix`, `MatchesExactly` or `FindTransformationSequences`, with `LongestPipingFlowAt` for a flow nested in an expression the rule already holds. Steps are matched on `Callee()` with the `IsNodeReferenceTo…` helpers. One flow predicate covers `pipe(x, f)`, `x.pipe(f)`, `f(x)`, data-first, data-last and curried spellings; a hand-matched call shape covers only the spelling its author thought of.

An AST walk (`ForEachChild`, a visitor) is right when the rule looks for something that never becomes a flow step: classes, `yield`, `new`, `return`, type nodes, zero-argument calls, or scans for references inside a body. `pnpm lint` requires each walk to carry an `//ets:ast-walk <reason>` comment. Review the reason: it must name what the walk finds that flows do not produce. A reason like "find the Effect.x calls" fails, because a flow predicate does that.

Flag:

- A walk, or a climb up `node.Parent`, that finds calls a flow step would produce.
- A second detection path beside the flow loop for one spelling, such as a direct `Effect.succeed(...)` branch.
- A switch or `||` chain on `Kind()` next to a callee check that already excludes the other kinds.
- A `TransformationInputNode(i) == nil` early return in a flow that can have `pipe` or `pipeable` steps; it silently drops the `.pipe(...)` spellings. Use `TransformationInputType(i)` or match the previous step.

## Fixes rewrite the flow

A fixable whose match holds a flow edits it with `ReplacePipingFlowPrefix`, `ReplacePipingFlowTransformation`, `CollapsePipingFlowTransformations` or `RemoveTrailingPipingFlowTransformation`. These keep the steps around the edit and every spelling of the input. `tracker.ReplaceNode` or raw text edits on step nodes are right only when no flow edit expresses the change, or when a flow edit would lose comments the fix must keep; the code says which. If no fix can be written well, the rule ships as a diagnostic only.

## Primitive gaps go to the user

A **primitive gap** is something flows or the rewriter cannot express yet: a missing step kind, a nil input node, an edit that drops arguments, a scan more than one rule would need. When an implementation hits one, the agent stops and raises it with the user before writing a workaround. It says what is missing, which rules would use it, and whether it looks like a parser bug or a new shared helper. The user decides whether to fix the parser, add the helper, or accept a local workaround.

Flag a local fallback, a "kind not supported" guard, or a private helper in a rule or fixable that works around a gap with no user decision on record in the PR. Agreed gaps are listed under "Known gaps" in `docs/agents/piping-flows.md`.

## Scope stays small

A rule handles the shapes its issue and fixtures name. Flag guards and branches for spellings no fixture covers and the issue does not mention: following identifiers to their declarations, `const p = Effect.provide` aliases, rarely written callee forms. Each one is code to port, test and maintain for a case nobody reported.

## Tests are fixtures

Behaviour is tested with fixtures under `testdata/tests/effect-v{3,4}/` and their baselines (`.errors.txt`, `.quickfixes.txt`, `.pipings.txt`), which both the Go and Rust runners check. Flag a new Go test for something a fixture can express, and any test that re-compiles applied fixes: the `.quickfixes.txt` baseline is the evidence. A fixture triggers only the rule under test, and no other rule, unless that interaction is the point.

## Claims match the code

- Comments about what flows or the rewriter support go stale as the primitives grow. Check the code (`internal/typeparser/piping_flow.go`, `internal/rewriter/piping_flow.go`), not the comment.
- A changeset, PR description or code comment describes what the diff does now. Flag mentions of a quick fix the rule no longer has, shapes it does not report, or tests the PR removed.
- A subagent's verdict that changes the design is checked against the code, open PRs and fixtures before it is acted on or passed to the user.
