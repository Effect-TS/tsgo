---
"@effect/tsgo": patch
---

Publish the packaged `lib/tsc` and `lib/tsc-next` binaries with mode `0755`.

`pnpm pack`/`pnpm publish` normalises every packed file to `0644` and grants `0755` only to files
referenced by the published manifest's `bin` field, ignoring the on-disk mode. Every published
`@effect/tsgo-*` tarball therefore shipped non-executable binaries, so `effect-tsgo diagnostics`
failed with `spawnSync ... EACCES` and `effect-tsgo get-exe-path` printed a path that consumers
could not execute. The POSIX platform packages now declare `publishConfig.bin` for both binaries,
which is what makes pnpm mark them executable at pack time.
