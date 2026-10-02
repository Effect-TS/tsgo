---
"@effect/tsgo": minor
---

Add `allowedUnstableApis` and `allowedExperimentalApis` to selectively allow unstable or experimental declaration modules or exported APIs. For example, `effect/http/HttpClient` allows the entire module, while `effect/http/HttpClient#get` allows only its exported `get` API. Both options support per-file overrides, and both diagnostics display the declaration-based name.
