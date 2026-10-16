---
"@effect/tsgo": patch
---

Make `apiStabilityLeak` faster on namespace exports. The rule checks each member of a namespace export in one shared analysis, and every member used to re-check every unresolved result that earlier members left behind. A namespace with thousands of members did millions of repeated checks. Each member now re-checks only the results its own check reached.

For example, on the Effect monorepo the `Generated` namespace export of `@effect/ai-openai` has 3,603 members. Across the whole repository the rule used 22.8 s of CPU before and 3.4 s after.

Reported diagnostics are unchanged.
