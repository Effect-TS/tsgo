---
"@effect/tsgo": patch
---

Fix repoctl spawning `npm`/`pnpm`/`corepack` on Windows after the Effect 4.1.2 update: the `.cmd` shims now run as a single shell command line, because the spawner no longer accepts `shell` together with an argument list. This unblocks the win32 release builds.
