---
"@effect/tsgo": patch
---

Fix the tsrs release build on Linux and macOS. tsrs's PGO training installs its bench projects with `pnpm --pm-on-fail=ignore`, which this repository's pnpm 10 rejects. The training now runs with the pnpm version pinned in tsrs's `package.json`, installed into the release work directory. The rest of the release job keeps the repository's own pnpm.
