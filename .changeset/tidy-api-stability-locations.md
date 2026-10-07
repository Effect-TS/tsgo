---
"@effect/tsgo": minor
---

Add related declaration locations to `apiStabilityLeak` diagnostics. For example, when a stable export exposes an experimental type, the diagnostic on the export now includes a child pointing to the experimental type's declaration, including declarations in other files.
