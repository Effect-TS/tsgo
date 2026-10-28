---
"@effect/tsgo": patch
---

Perf CI: base and head are now measured on the same runner, and only when a maintainer opts in with the `perf` label. Builds still run in parallel, one job per channel and side, each with that commit's own tooling. A separate `measure` job per channel downloads both binaries and runs 9 rounds of stock, base and head compiles in a rotating order. The report's main column is now head vs base directly, flagged by a paired Wilcoxon signed-rank test (p < 0.01), with base and PR overhead over stock tsgo kept as context. `repoctl perf ci` takes `--base-bin/--base-commit/--base-version`, `--head-bin/--head-commit/--head-version` and `--rounds` instead of `--side/--commit/--version/--patched-bin/--pairs`, and writes version 2 cells:

```sh
repoctl perf ci --channel latest \
  --base-bin ./tsgo-base --base-commit "$BASE_SHA" --base-version 7.0.2 \
  --head-bin ./tsgo-head --head-commit "$HEAD_SHA" --head-version 7.0.2 \
  --workload-dir /tmp/effect-workload --output perf/latest.json
```
