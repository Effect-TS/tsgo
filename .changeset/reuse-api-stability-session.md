---
"@effect/tsgo": patch
---

Speed up `apiStabilityLeak` by reusing one stability analysis per file.

The rule used to allocate a fresh analysis, with its maps and work queues, for every export of a file. It now keeps one analysis per file and resets it before each export. The reset analysis behaves exactly like a fresh one, so the reported diagnostics are unchanged. A published surface that a later export reads from the shared cache is no longer queued for publication again, and a published surface without retained findings no longer allocates a findings map.

On `packages/effect` of effect 4.0.5 with every rule enabled, the rule allocates 252 MB → 140 MB.
