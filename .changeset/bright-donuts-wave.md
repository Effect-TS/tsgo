---
"@effect/tsgo": patch
---

Fix execution-flow graph generation for single-argument inline calls such as `Layer.succeed(Service)(value)`.

This updates the flow parser to connect inline call subjects and transforms correctly, and refreshes the generated reference baselines and metadata outputs to match the new local results.
