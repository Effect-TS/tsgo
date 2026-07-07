---
"@effect/tsgo": patch
---

Fix the release workflow embedding a stale `EffectVersion` in the published `tsc` binary. The version bump from the changeset release PR only lands on `main`, while the `tsc` binary builds from `generated/stable`; the workflow now syncs `_packages/tsgo/package.json` from the release merge commit and re-runs `_tools/version-prepare.sh` before building. All release checkouts are also pinned to the merge commit SHA instead of the moving `main` ref so the release is deterministic.
