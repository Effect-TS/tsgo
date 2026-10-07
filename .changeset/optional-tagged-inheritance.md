---
"@effect/tsgo": patch
---

Fix `apiStabilityLeak` to ignore inherited bases whose public members are all optional and explicitly stability-tagged, including bases reached through augmentations and superclass factories. Direct properties and parameters typed as those bases remain checked. Exclude `@internal` properties and methods from the public surface throughout the leak check.
