---
"@effect/tsgo": patch
---

Add a Perf workflow that compares each pull request against its base for every supported TypeScript channel. Each job runs stock and Effect-patched tsgo on a pinned Effect checkout with every rule executing, and a sticky PR comment reports the change in symbols, types, instantiations, allocations, and check and total time over stock. Run it locally with `repoctl perf ci` and `repoctl perf report`.
