# Shim overlays for TypeScript beta

Per-commit override of `../typescript/` (see `_patches/typescript/README.md`) for
TypeScript `7.1.0-beta` (`3e9236361fa97a9e6c685f4833ac457ae2d8c50a`). When that
commit is set up, this folder is the only shim overlay and the shims are generated
through the backport prefix, so it is self-contained: a copy of every package of
`../typescript/`, plus the `tsoptions/compatibility.go` adapters that present the
canonical (next) `tsoptions` API over the beta compiler.

A change to `../typescript/` must be mirrored here in the same pull request
(nothing checks it). When beta moves to a new commit, `repoctl check` reports
this folder as stale: rename it to the new `gitHead` (adapting the adapters), or
delete it when the canonical shims apply to the new beta.
