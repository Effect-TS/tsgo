---
"@effect/tsgo": patch
---

Fix the toggle-pipe-style refactor to avoid formatter panics on nested callback bodies such as SQL effects using `.pipe(Effect.flatMap(...))`.

This adds a regression test and updates the affected refactor baselines to match the new text-preserving rewrite output.
