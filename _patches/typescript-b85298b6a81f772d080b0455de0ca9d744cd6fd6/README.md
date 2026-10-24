# TypeScript patches backported for tsrs

This folder is `_patches/typescript` backported to the TypeScript commit that the
pinned tsrs release ports (the tsrs component's `typescript` dependency in
`_packages/tsgo/upstream.json`; tsrs 0.18.0 ports TypeScript `7.1.0-dev.20260929`,
`b85298b6a81f772d080b0455de0ca9d744cd6fd6`). It is a per-commit override folder
(see `_patches/typescript/README.md`): its name ends with that `gitHead`, so
`pnpm exec repoctl submodules setup --component tsrs` (and `--component
typescript --version 7.1.0-dev.20260929`) applies these patches instead of
`_patches/typescript`, in bytewise filename order, with zero fuzz. Its Go shims
come from the matching override folder
`_tools/gen_shims/providers/typescript-b85298b6a81f772d080b0455de0ca9d744cd6fd6`.

It mirrors `_patches/typescript` patch for patch: the same file names and numbers,
each one the same change re-made against that older TypeScript, with the same
behavior and the same exported API that the Effect Go code and the Go shims rely
on. A patch that applies unchanged is a byte-for-byte copy; the others are real
diffs generated in a checkout of that commit. `repoctl check` fails when the two
folders do not hold the same patch files.

Any change to `_patches/typescript` must be mirrored here in the same pull
request (and into `_patches/tsrs` where it maps). To re-make a patch: check out
that commit (`pnpm exec repoctl submodules setup --component tsrs`, which applies
the folder up to the failing patch), apply the earlier patches, make the change
in `typescript/`, and write `git -C typescript diff` to the patch file; then run
`pnpm exec repoctl test` with that setup.

When tsrs moves to another TypeScript, `repoctl check` reports this folder (and
its shim folder) as stale: rename both to the new `gitHead` and re-make what no
longer applies, or delete both once `_patches/typescript` and the canonical shims
apply to the new commit.
