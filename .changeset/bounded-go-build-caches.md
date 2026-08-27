---
"@effect/tsgo": patch
---

Bound CI Go caches with a resolved cache identity

`repoctl upstream resolve` now emits a Go cache identity derived from `upstream.json`: the `typescript` component owns and saves its Go caches, `oxlint-tsgolint` restores the compiler objects already cached by the typescript jobs under its TypeScript dependency version (Go's build cache is content-addressed, so the shared compiler packages hit directly), and the Rust-based `oxlint` component skips Go caching entirely instead of saving a duplicate GOCACHE snapshot.

The go-build cache key no longer includes the commit SHA, so a multi-GB cache entry is written once per dependency state instead of on every push to main, and non-materialized setups are restore-only. Previously each push saved ~15GB of duplicate GOCACHE snapshots, evicting every other cache in the repository (GitHub caps repository caches at 10GB) and forcing all validation jobs to run cold.

The generated shim cache key now also hashes `_patches/typescript/**`, so patch changes for the migrated TypeScript compiler repository invalidate cached shims correctly.
